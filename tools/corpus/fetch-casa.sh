#!/usr/bin/env bash
# Fetch the CASA image corpus into tools/corpus/casa/ (gitignored: nothing here is committed)
# and write python-casacore's reading of it (the oracle) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-casa.sh
#   OUT defaults to $SPECULAR_CASA_CORPUS, else tools/corpus/casa.
#   PYTHON (default python3) must have python-casacore and numpy for the oracle; a venv under
#   target/ is fine: python3 -m venv target/casa/venv &&
#   target/casa/venv/bin/pip install --no-cache-dir python-casacore numpy; then
#   PYTHON=target/casa/venv/bin/python.
#   Then: SPECULAR_CASA_DIR=tools/corpus/casa cargo test -p specular-casa --test oracle
#   corpus_matches_python_casacore
#
# What it covers (under 1 MB; real CASA-written image tables are rarely published on their
# own, tclean products ship inside multi-GB tutorial tarballs):
#   casacore/      images written by casacore's own test programs at a pinned commit
#                  (LGPL-3.0): a 2-D deconvolution test image and a 4-D test image
#   spectral-cube/ spectral-cube's CASA test images at a pinned commit (BSD-3-Clause): a
#                  little-endian cube with a default mask, the same cube big-endian, and one
#                  without a mask
# Each image is its whole directory: table.dat, table.f0, table.f0_TSM0, the mask0 subtable and
# the logtable history subtable (our reader never opens it, python-casacore refuses an image
# without it).
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_CASA_CORPUS:-$ROOT/tools/corpus/casa}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
CASACORE=a1399b6b99907bd14021920a089403a93c10aba1
SPECTRAL_CUBE=1882ac2addaf2786b800654b0c319eb268be82db
mkdir -p "$OUT/casacore" "$OUT/spectral-cube"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-casa: $*" >&2; exit 1; }

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
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ ! -s "$OUT/$dest" ]]; then
    if ! curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url"; then
      rm -f "$OUT/$dest.part"
      die "download failed: $url"
    fi
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

# image BASE_URL DEST_DIR LICENCE SUBDIR...: an image table and its logtable; each SUBDIR
# ("." for the image itself, "mask0" for its mask) gives table.dat, table.f0, table.f0_TSM0 and
# table.info.
image() {
  local base=$1 dest=$2 lic=$3
  shift 3
  local sub f
  for sub in "$@"; do
    for f in table.dat table.f0 table.f0_TSM0 table.info; do
      get "$base/$sub/$f" "$dest/$sub/$f" "$lic"
    done
  done
  for f in table.dat table.f0 table.info; do
    get "$base/logtable/$f" "$dest/logtable/$f" "$lic"
  done
}

CC=https://raw.githubusercontent.com/casacore/casacore/$CASACORE/images/Images/test
lic="LGPL-3.0 (casacore test data)"
image "$CC/decon_test.im" casacore/decon_test.im "$lic" .
image "$CC/test_image.im" casacore/test_image.im "$lic" .

SC=https://raw.githubusercontent.com/radio-astro-tools/spectral-cube/$SPECTRAL_CUBE/spectral_cube/tests/data
lic="BSD-3-Clause (spectral-cube test data)"
image "$SC/basic.image" spectral-cube/basic.image "$lic" . mask0
image "$SC/basic_bigendian.image" spectral-cube/basic_bigendian.image "$lic" . mask0
image "$SC/nomask.image" spectral-cube/nomask.image "$lic" .

# The oracle: python-casacore's reading of every image, named by directory.
"$PYTHON" -I "$ROOT/crates/formats/specular-casa/gen/oracle.py" "$OUT/casacore" "$OUT/spectral-cube" \
  > "$OUT/expected.tsv" || die "oracle failed (does PYTHON have python-casacore and numpy?)"
du -sh "$OUT"
