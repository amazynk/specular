#!/usr/bin/env bash
# Fetch the sidecar and gridded-raster corpus into tools/corpus/geosidecar/ (gitignored:
# nothing here is committed). The specular-decode test `geosidecar::corpus_matches_gdal`
# reads it in place from SPECULAR_GEOSIDECAR_CORPUS (a worktree has no corpus of its own),
# else from tools/corpus/geosidecar, and skips when it is absent.
#
# Usage: tools/corpus/fetch-geosidecar.sh [SECTION...]
#   SECTION is a function name below (e.g. src_gdal). Default: all sections, then the
#   oracle (src_oracle) when PYTHON has rasterio and pyproj.
#   OUT defaults to $SPECULAR_GEOSIDECAR_CORPUS, else tools/corpus/geosidecar.
#   PYTHON (default python3) is for the oracle only; a venv from wheels is enough:
#     python3 -m venv target/gis-venv
#     target/gis-venv/bin/pip install --no-cache-dir --only-binary=:all: pyproj rasterio
#   then PYTHON=target/gis-venv/bin/python.
#
# What it covers (about 150 MB):
#   gdal      GDAL's autotest data at tag v3.11.0 (MIT/X11): Esri ASCII grids, DTED (with
#             broken and partial tiles), ENVI (BSQ/BIL/BIP, rotated, big-endian, classes),
#             ESRI .hdr rasters, ERDAS IMAGINE (RLE, external spill files, projections) and
#             world-file pictures
#   usgs      a USGS 3DEP 1 arc-second Esri ASCII grid with its .prj and .aux.xml (public
#             domain), about 50 MB: the timing case
#   enmap     the EnMAP-Box Potsdam EnMAP scene (CC BY 4.0, 224 bands), which the oracle
#             converts with GDAL into ENVI BSQ and BIL with wavelengths
#   expected/ the oracle: per file, rasterio's size, transform, EPSG, nodata, wavelengths
#             and values, and pyproj's longitude/latitude of the centre pixel
#
# Downloads are untrusted data: this script only copies files, never runs anything from
# them (the oracle reads them from a script outside the corpus). Scratch space is
# target/corpus/geosidecar-fetch (deleted afterwards). Stops if free space drops below
# MIN_FREE_GB (default 6), or the corpus would pass 500 MB.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_GEOSIDECAR_CORPUS:-$ROOT/tools/corpus/geosidecar}
SCRATCH="$ROOT/target/corpus/geosidecar-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
MAX_MB=500
PYTHON=${PYTHON:-python3}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-geosidecar: $*" >&2; exit 1; }

check_space() {
  local avail_kb used_mb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
  used_mb=$(du -sm "$OUT" | cut -f1)
  (( used_mb < MAX_MB )) || die "the corpus is ${used_mb} MB (>= ${MAX_MB} MB); stopping"
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# fetch URL DEST LICENCE [SIZE]: one file, its size checked when given
fetch() {
  local url=$1 dest=$2 lic=$3 size=${4:-}
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  curl -fsSL --max-time 900 -o "$OUT/$dest" "$url"
  if [[ -n $size && $(stat -c %s "$OUT/$dest") != "$size" ]]; then
    die "$dest: size $(stat -c %s "$OUT/$dest"), expected $size"
  fi
  provenance "$dest" "$url" "$lic"
  echo "  $dest"
}

# ------------------------------------------------------------------------------- sections

GDAL_COMMIT=447eb5238bb6ef2837e68bf2ec742c64007b680b   # tag v3.11.0

src_gdal() {         # GDAL autotest data (MIT/X11)
  local work="$SCRATCH/gdal" n=0 f rel
  local lic="MIT/X11 (GDAL autotest)"
  check_space
  rm -rf "$work"
  git init -q "$work"
  git -C "$work" remote add origin https://github.com/OSGeo/gdal
  git -C "$work" config core.sparseCheckout true
  git -C "$work" sparse-checkout set --no-cone \
    '/autotest/gdrivers/data/aaigrid/*' '/autotest/gdrivers/data/dted/*' \
    '/autotest/gdrivers/data/n43.dt0' '/autotest/gdrivers/data/envi/*' \
    '/autotest/gdrivers/data/ehdr/*' '/autotest/gdrivers/data/hfa/*' \
    '/autotest/gdrivers/data/*.wld' '/autotest/gdrivers/data/*.jgw' \
    '/autotest/gdrivers/data/*.tfw' '/autotest/gdrivers/data/*.pgw' \
    '/autotest/gdrivers/data/png/*' '/autotest/gdrivers/data/jpeg/*'
  git -C "$work" fetch -q --depth 1 --filter=blob:none origin "$GDAL_COMMIT"
  git -C "$work" -c advice.detachedHead=false checkout -q FETCH_HEAD
  while IFS= read -r -d '' f; do
    rel=${f#"$work/autotest/gdrivers/data"/}
    mkdir -p "$(dirname "$OUT/gdal/$rel")"
    cp "$f" "$OUT/gdal/$rel"
    provenance "gdal/$rel" "https://github.com/OSGeo/gdal/blob/$GDAL_COMMIT/autotest/gdrivers/data/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work/autotest/gdrivers/data" -type f -print0)
  rm -rf "$work"
  echo "  gdal: $n files"
  check_space
}

USGS=https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/1AS_ASCII
src_usgs() {         # USGS 3DEP (public domain)
  local lic="public domain (USGS 3DEP)"
  fetch "$USGS/n16w093.asc" usgs/n16w093.asc "$lic" 51430777
  fetch "$USGS/n16w093.prj" usgs/n16w093.prj "$lic"
  fetch "$USGS/n16w093.asc.aux.xml" usgs/n16w093.asc.aux.xml "$lic"
}

ENMAP_COMMIT=main
src_enmap() {        # EnMAP-Box example data (CC BY 4.0, see its readme.txt)
  local base="https://raw.githubusercontent.com/EnMAP-Box/enmap-box/$ENMAP_COMMIT/enmapbox/exampledata"
  fetch "$base/enmap_potsdam.tif" enmap/enmap_potsdam.tif "CC BY 4.0 (EnMAP-Box example data)" 29367591
  fetch "$base/readme.txt" enmap/readme.txt "CC BY 4.0 (EnMAP-Box example data)"
}

src_oracle() {       # rasterio/pyproj expectations, and the ENVI conversions
  if ! "$PYTHON" -I -c "import pyproj, rasterio" 2>/dev/null; then
    echo "  $PYTHON lacks pyproj/rasterio; no oracle (see the header for a venv)"
    return
  fi
  "$PYTHON" -I "$ROOT/crates/formats/specular-gis/gen/expect_geosidecar_corpus.py" "$OUT" --convert
}

main() {
  if (( $# == 0 )); then
    set -- src_gdal src_usgs src_enmap src_oracle
    rm -f "$OUT/.provenance.tsv"
  fi
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "unknown section $s"
    echo "$s"
    "$s"
  done
  rm -rf "$SCRATCH"
  echo "corpus: $(du -sh "$OUT" | cut -f1) in $OUT"
}

main "$@"
