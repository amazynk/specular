#!/usr/bin/env bash
# The 3D Tiles corpus for specular-tiles3d's corpus test and tools/corpus/tiles3d-oracle.py.
#
# Sources (local use only, never committed):
#   CesiumGS/cesium, Specs/Data/Cesium3DTiles: Apache-2.0 (the repository's LICENSE.md).
#   CesiumGS/3d-tiles-samples, 1.0 and 1.1: per-sample licences in each README (CC0, CC-BY 4.0).
#
# Usage: tools/corpus/fetch-3dtiles.sh [DEST]   (default tools/corpus/tiles3d)
# Then:  SPECULAR_TILES3D_CORPUS=DEST cargo test -p specular-tiles3d --test corpus -- --nocapture
#        python3 tools/corpus/tiles3d-oracle.py DEST DEST
#        (and the test again: it compares with oracle.json)
set -euo pipefail
dest="${1:-$(dirname "$0")/tiles3d}"
mkdir -p "$dest"
dest="$(cd "$dest" && pwd)"
work="$dest/.git-work"
rm -rf "$work"
mkdir -p "$work"

sparse() { # repo dir paths...
  local repo="$1" dir="$2"
  shift 2
  echo "Fetching $repo ($*)"
  git clone --progress --depth 1 --filter=blob:none --sparse "https://github.com/$repo.git" "$work/$dir"
  git -C "$work/$dir" sparse-checkout set "$@"
}

sparse CesiumGS/cesium cesium Specs/Data/Cesium3DTiles
sparse CesiumGS/3d-tiles-samples samples 1.0 1.1

rm -rf "$dest/cesium" "$dest/samples"
mkdir -p "$dest/cesium" "$dest/samples"
cp -r "$work/cesium/Specs/Data/Cesium3DTiles/." "$dest/cesium/"
cp -r "$work/samples/1.0" "$work/samples/1.1" "$dest/samples/"
rm -rf "$work"
du -sh "$dest"
