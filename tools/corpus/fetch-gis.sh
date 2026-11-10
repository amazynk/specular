#!/usr/bin/env bash
# Fetch the geospatial test corpus into tools/corpus/gis/ (gitignored: nothing here is
# committed). The specular-gis corpus tests read it in place from SPECULAR_GIS_CORPUS (a
# worktree has no corpus of its own), else from tools/corpus/gis, and skip when it is absent.
#
# Usage: tools/corpus/fetch-gis.sh [SECTION...]
#   SECTION is a function name below (e.g. src_gdal). Default: all sections, then the oracle
#   (src_oracle) when PYTHON has pyproj, pyogrio, shapely and rasterio.
#   OUT defaults to $SPECULAR_GIS_CORPUS, else tools/corpus/gis.
#   PYTHON (default python3) is for the oracle only. A venv under target/ from wheels alone
#   (no system packages) is enough:
#     python3 -m venv target/gis-venv
#     target/gis-venv/bin/pip install --no-cache-dir --only-binary=:all: pyproj shapely pyogrio rasterio
#   then PYTHON=target/gis-venv/bin/python.
#
# What it covers (about 250 MB):
#   gdal      GDAL's autotest data at tag v3.11.0 (MIT/X11 and per-file notes): GeoTIFFs and
#             COGs (gcore/data, gdrivers/data/gtiff), Shapefiles with every shape type,
#             encodings and broken files (ogr/data/shp), GeoPackages with features and
#             tiles (ogr/data/gpkg, gdrivers/data/gpkg), KML, GPX and GeoJSON (ogr/data)
#   ne        Natural Earth vectors (public domain), sizes and SHA-256 checked: 110m
#             countries, 10m coastline, land, roads, populated places, admin-1 (a 50 MB-class
#             Shapefile for timings)
#   expected/ the oracle: per vector file, pyogrio's layers, feature counts, field names,
#             attributes and coordinates; per GeoTIFF, rasterio's CRS, transform and nodata;
#             pyproj's longitude/latitude of sample points
#
# Downloads are untrusted data: this script only copies files, never runs anything from
# them (the oracle reads them from a script outside the corpus). Scratch space is
# target/agent-corpus/gis-fetch (deleted afterwards). Stops if free space on the target
# filesystem drops below MIN_FREE_GB (default 6), or the corpus would pass 1 GB.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_GIS_CORPUS:-$ROOT/tools/corpus/gis}
SCRATCH="$ROOT/target/agent-corpus/gis-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
MAX_MB=1024
PYTHON=${PYTHON:-python3}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-gis: $*" >&2; exit 1; }

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

# sparse REPO COMMIT PATTERN... -> prints the checkout folder
sparse() {
  local repo=$1 commit=$2; shift 2
  local work="$SCRATCH/${repo//\//_}"
  check_space
  rm -rf "$work"
  git init -q "$work"
  git -C "$work" remote add origin "https://github.com/$repo"
  git -C "$work" config core.sparseCheckout true
  git -C "$work" sparse-checkout set --no-cone "$@"
  git -C "$work" fetch -q --depth 1 --filter=blob:none origin "$commit"
  git -C "$work" -c advice.detachedHead=false checkout -q FETCH_HEAD
  echo "$work"
}

# copy_tree WORK SUBDIR DEST REPO COMMIT LICENCE
copy_tree() {
  local work=$1 sub=$2 dest=$3 repo=$4 commit=$5 lic=$6 n=0
  [[ -d "$work/$sub" ]] || return 0
  while IFS= read -r -d '' f; do
    local rel=${f#"$work/$sub"/}
    mkdir -p "$(dirname "$OUT/$dest/$rel")"
    cp "$f" "$OUT/$dest/$rel"
    provenance "$dest/$rel" "https://github.com/$repo/blob/$commit/$sub/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work/$sub" -type f -print0)
  echo "  $repo/$sub: $n files -> $dest"
}

# ------------------------------------------------------------------------------- sections

GDAL_COMMIT=447eb5238bb6ef2837e68bf2ec742c64007b680b   # tag v3.11.0

src_gdal() {         # GDAL autotest data (MIT/X11; some files carry their own notes)
  local work
  work=$(sparse OSGeo/gdal "$GDAL_COMMIT" \
    '/autotest/gcore/data/*.tif' '/autotest/gcore/data/cog/*' \
    '/autotest/gdrivers/data/gtiff/*.tif' \
    '/autotest/ogr/data/shp/*' '/autotest/ogr/data/poly.*' \
    '/autotest/ogr/data/gpkg/*.gpkg' '/autotest/gdrivers/data/gpkg/*.gpkg' \
    '/autotest/ogr/data/kml/*.kml' '/autotest/ogr/data/kml/*.kmz' \
    '/autotest/ogr/data/gpx/*.gpx' \
    '/autotest/ogr/data/geojson/*.json' '/autotest/ogr/data/geojson/*.geojson')
  local lic="MIT/X11 (GDAL autotest; see the file's own notes where present)"
  copy_tree "$work" autotest/gcore/data gdal/gcore OSGeo/gdal "$GDAL_COMMIT" "$lic"
  copy_tree "$work" autotest/gdrivers/data gdal/gdrivers OSGeo/gdal "$GDAL_COMMIT" "$lic"
  copy_tree "$work" autotest/ogr/data gdal/ogr OSGeo/gdal "$GDAL_COMMIT" "$lic"
  rm -rf "$work"
  check_space
}

# name size sha256 (Natural Earth 5.1.x on its CDN; checked, so a changed file fails loudly)
NE_FILES=(
  "110m/cultural/ne_110m_admin_0_countries.zip 214976 0f243aeac8ac6cf26f0417285b0bd33ac47f1b5bdb719fd3e0df37d03ea37110"
  "10m/physical/ne_10m_coastline.zip 3069451 bfa04cdbcbef07ef90dfca1dabb48062eca29900a113df0f389303e255484017"
  "10m/physical/ne_10m_land.zip 3269070 e547d749445eaa0964aba76738090ec88f5e63c4585122170f98c67a7ea922dc"
  "10m/cultural/ne_10m_roads.zip 9075919 880b235383a1e1ccf33f4ea6f82b6755a2ffbce07e3974e412b43e88fe954ae9"
  "10m/cultural/ne_10m_populated_places.zip 2811199 cd149186f03d2603e0410da399b980a4357d0ac32d3a2305a49ed3dffcc41d7b"
  "10m/cultural/ne_10m_admin_1_states_provinces.zip 14909524 efc59726337323058f9446210adc96673179cd344e053666ee3d28cb58ba2b05"
)

src_ne() {           # Natural Earth (public domain)
  mkdir -p "$OUT/ne"
  local entry rel size sum zip name
  for entry in "${NE_FILES[@]}"; do
    read -r rel size sum <<< "$entry"
    check_space
    name=$(basename "$rel" .zip)
    zip="$SCRATCH/$name.zip"
    curl -fsSL --max-time 300 -o "$zip" "https://naciscdn.org/naturalearth/$rel"
    [[ $(stat -c %s "$zip") == "$size" ]] || die "$rel: size $(stat -c %s "$zip"), expected $size"
    if [[ $sum != - ]]; then
      [[ $(sha256sum "$zip" | cut -d' ' -f1) == "$sum" ]] || die "$rel: SHA-256 mismatch"
    fi
    mkdir -p "$OUT/ne/$name"
    unzip -q -o -j "$zip" -d "$OUT/ne/$name" '*.shp' '*.shx' '*.dbf' '*.prj' '*.cpg'
    for f in "$OUT/ne/$name"/*; do
      provenance "ne/$name/$(basename "$f")" "https://naciscdn.org/naturalearth/$rel" "public domain (Natural Earth)"
    done
    rm -f "$zip"
    echo "  ne/$name"
  done
}

src_oracle() {       # pyogrio/rasterio/pyproj expectations (skipped without them)
  if ! "$PYTHON" -I -c "import pyproj, pyogrio, shapely, rasterio" 2>/dev/null; then
    echo "  $PYTHON lacks pyproj/pyogrio/shapely/rasterio; no oracle (see the header for a venv)"
    return
  fi
  "$PYTHON" -I "$ROOT/crates/formats/specular-gis/gen/oracle.py" "$OUT"
}

main() {
  if (( $# == 0 )); then
    set -- src_gdal src_ne src_oracle
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
