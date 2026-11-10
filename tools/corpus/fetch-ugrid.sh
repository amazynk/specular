#!/usr/bin/env bash
# Fetch a small UGRID / ICON unstructured-mesh corpus into tools/corpus/ugrid/ (gitignored:
# nothing here is committed) and write xugrid's reading of it (the oracle) to expected.tsv.
#
# Usage: tools/corpus/fetch-ugrid.sh
#   OUT defaults to $SPECULAR_UGRID_CORPUS, else tools/corpus/ugrid.
#   PYTHON (default python3) must have netCDF4, numpy, xugrid and numba_celltree; a venv
#   under target/ is fine: python3 -m venv target/ugrid/venv &&
#   target/ugrid/venv/bin/pip install --no-cache-dir netCDF4 xarray xugrid; then
#   PYTHON=target/ugrid/venv/bin/python (NUMBA_CACHE_DIR under target/ too).
#   Then: SPECULAR_UGRID_DIR=tools/corpus/ugrid cargo test -p specular-ugrid --test oracle
#
# What it covers (about 25 MB), all saved with a .nc extension:
#   xugrid sample data (MIT): ADH_SanDiego (an ADH estuary triangle mesh, projected metres,
#     node elevation and face data) and elevation_nl (a Netherlands triangle mesh)
#   uxarray test meshes (Apache-2.0): a cubed-sphere ne30 grid, a quad-hexagon grid with
#     face data, a FESOM ocean mesh, an overlap mesh RLL10deg x CSne4 and the ICON R02B04
#     global grid (vertex_of_cell over vlon/vlat in radians, 20480 triangles) and an MPAS
#     quasi-uniform 1920 km Voronoi mesh (verticesOnCell over lonVertex/latVertex)
#   NOAA gridded test data (public domain US government work): UGRID 0.9 eleven points
#     (with and without depth) and a mixed quad and triangle mesh
# No small public FVCOM, ADCIRC or SCHISM sample with a stable URL was found; the tests
# build FVCOM-style files (nv nodes-by-faces, 1-based without start_index) synthetically.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_UGRID_CORPUS:-$ROOT/tools/corpus/ugrid}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
XUGRID=https://github.com/Deltares/xugrid/raw/main/data
UX=https://github.com/UXARRAY/uxarray/raw/main/test/meshfiles
GRIDDED=https://github.com/NOAA-ORR-ERD/gridded/raw/master/gridded/tests
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-ugrid: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE [optional]: an optional file missing upstream is skipped.
get() {
  local url=$1 dest=$2 lic=$3 optional=${4:-}
  check_space
  if [[ ! -s "$OUT/$dest" ]]; then
    if ! curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url"; then
      rm -f "$OUT/$dest.part"
      [[ -n $optional ]] && return 0
      die "download failed: $url"
    fi
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

lic="MIT (xugrid sample data)"
get "$XUGRID/ADH_SanDiego.nc" adh_sandiego.nc "$lic"
get "$XUGRID/elevation_nl.nc" elevation_nl.nc "$lic"
lic="Apache-2.0 (uxarray test meshes)"
get "$UX/ugrid/outCSne30/outCSne30.ug" ux_outCSne30.nc "$lic"
get "$UX/ugrid/quad-hexagon/grid.nc" ux_quad_hexagon.nc "$lic"
get "$UX/ugrid/fesom/fesom.mesh.diag.nc" ux_fesom.nc "$lic"
get "$UX/ugrid/ov_RLL10deg_CSne4/ov_RLL10deg_CSne4.ug" ux_ov_RLL10deg_CSne4.nc "$lic"
get "$UX/icon/R02B04/icon_grid_0010_R02B04_G.nc" icon_grid_R02B04.nc "$lic"
get "$UX/mpas/QU/mesh.QU.1920km.151026.nc" mpas_QU_1920km.nc "$lic"
lic="public domain (NOAA ORR-ERD gridded test data)"
get "$GRIDDED/test_data/UGRIDv0.9_eleven_points.nc" gridded_eleven_points.nc "$lic"
get "$GRIDDED/test_data/UGRIDv0.9_eleven_points_with_depth.nc" gridded_eleven_points_depth.nc "$lic"
get "$GRIDDED/test_ugrid/files/quad_and_triangle.nc" gridded_quad_and_triangle.nc "$lic" optional

# The oracle: xugrid's face under sampled points and the values there.
"$PYTHON" -I -W ignore "$ROOT/crates/formats/specular-ugrid/gen/oracle.py" "$OUT" \
  || die "oracle failed (does PYTHON have netCDF4, xugrid and numba_celltree?)"
du -sh "$OUT"
