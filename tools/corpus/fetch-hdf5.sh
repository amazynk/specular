#!/usr/bin/env bash
# Fetch and generate the HDF5 / NetCDF / EMD test corpus into tools/corpus/hdf5/ (gitignored:
# nothing here is committed). The tests themselves never need it: they generate small fixtures
# with h5py / netCDF4 (crates/formats/specular-hdf5/gen/make_fixtures.py) under target/. This
# corpus is for manual checks in the viewer and for timings.
#
# Usage: tools/corpus/fetch-hdf5.sh [SECTION...]
#   SECTION is a function name below (e.g. src_unidata). Default: all sections.
#   OUT defaults to $SPECULAR_HDF5_CORPUS, else tools/corpus/hdf5.
#   PYTHON (default: target/hdf5-venv/bin/python when present, else python3) needs numpy and
#   h5py, and netCDF4 for the NetCDF files. A venv under target/ is fine:
#   python3 -m venv target/hdf5-venv && target/hdf5-venv/bin/pip install --no-cache-dir \
#     h5py netCDF4 hdf5plugin
#
# What it covers:
#   unidata    Unidata's NetCDF example files (classic and NetCDF-4 climate grids)
#   generated  the test fixtures (every layout, chunk index and filter; EMD Berkeley and
#              Velox) and the speed files of gen/make_speed.py (about 150 MB)
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# A download that fails is reported and skipped. Stops if free space on the target filesystem
# drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_HDF5_CORPUS:-$ROOT/tools/corpus/hdf5}
MIN_FREE_GB=${MIN_FREE_GB:-6}
if [[ -z "${PYTHON:-}" ]]; then
  if [[ -x "$ROOT/target/hdf5-venv/bin/python" ]]; then
    PYTHON=$ROOT/target/hdf5-venv/bin/python
  else
    PYTHON=python3
  fi
fi
GEN="$ROOT/crates/formats/specular-hdf5/gen"
mkdir -p "$OUT"

die() { echo "fetch-hdf5: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# url_file URL DEST LICENCE
url_file() {
  check_space
  mkdir -p "$(dirname "$OUT/$2")"
  if curl -fsSL --retry 3 -o "$OUT/$2.part" "$1"; then
    mv "$OUT/$2.part" "$OUT/$2"
    provenance "$2" "$1" "$3"
  else
    rm -f "$OUT/$2.part"
    echo "fetch-hdf5: could not fetch $1; skipped" >&2
  fi
}

src_unidata() {
  local base=https://www.unidata.ucar.edu/software/netcdf/examples
  for f in sresa1b_ncar_ccsm3-example.nc ECMWF_ERA-40_subset.nc tos_O1_2001-2002.nc \
           test_echam_spectral-deflated.nc; do
    [[ -s "$OUT/unidata/$f" ]] || url_file "$base/$f" "unidata/$f" "Unidata example data"
  done
}

src_generated() {
  check_space
  mkdir -p "$OUT/generated"
  ( cd "$OUT/generated" && "$PYTHON" -I "$GEN/make_fixtures.py" "$OUT/generated" )
  "$PYTHON" -I "$GEN/make_speed.py" "$OUT/generated"
}

sections=("$@")
if (( ${#sections[@]} == 0 )); then
  sections=(src_unidata src_generated)
fi
for s in "${sections[@]}"; do
  declare -F "$s" > /dev/null || die "no section $s"
  echo "fetch-hdf5: $s"
  "$s"
done
echo "fetch-hdf5: done; corpus in $OUT"
