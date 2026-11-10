#!/usr/bin/env bash
# Fetch the 3D model (mesh) test corpus into tools/corpus/mesh/ (gitignored: nothing here is
# committed). The specular-mesh corpus test reads it in place from SPECULAR_MESH_CORPUS (a
# worktree has no corpus of its own), else from tools/corpus/mesh, and skips when it is absent.
#
# Usage: tools/corpus/fetch-mesh.sh [SECTION...]
#   SECTION is a function name below (e.g. src_khronos). Default: all sections, then the
#   trimesh oracle (src_oracle) when PYTHON has trimesh.
#   OUT defaults to $SPECULAR_MESH_CORPUS, else tools/corpus/mesh.
#   PYTHON (default python3) needs trimesh (and numpy) for the oracle only. A venv under
#   target/ is fine: python3 -m venv target/mesh-venv && target/mesh-venv/bin/pip install
#   --no-cache-dir trimesh lxml networkx; then PYTHON=target/mesh-venv/bin/python.
#
# What it covers (about 120 MB):
#   khronos   glTF-Sample-Assets at a pinned commit: the glTF (separate .bin and textures),
#             glTF-Binary and glTF-Embedded variants of a spread of models (textured, vertex
#             colours, many nodes, sparse accessors, odd orientations, large scenes)
#   trimesh   trimesh's test models at a pinned commit: STL (ASCII, binary, multi-solid, units in
#             the name), OBJ with MTL and textures (groups, negative indices, polygons, missing
#             parts), PLY (ASCII, binary, colours, UVs, points only, empty) and 3MF, including
#             files that are empty or broken on purpose
#   expected.tsv  trimesh's triangle counts and bounds for every file it reads (the oracle)
#
# Downloads are untrusted data: this script only copies files, never runs anything from them
# (the oracle reads them with trimesh, from a script outside the corpus). Scratch space is
# target/corpus/mesh-fetch (deleted afterwards). Stops if free space on the target
# filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MESH_CORPUS:-$ROOT/tools/corpus/mesh}
SCRATCH="$ROOT/target/corpus/mesh-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-python3}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-mesh: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# sparse REPO COMMIT PATTERN... -> prints the checkout folder
# Sparse, blobless, depth-1 fetch of PATTERN... at COMMIT.
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

# copy_tree WORK SUBDIR DEST REPO COMMIT LICENCE: copies WORK/SUBDIR/** to OUT/DEST/**,
# keeping the layout (side files are found next to their model).
copy_tree() {
  local work=$1 sub=$2 dest=$3 repo=$4 commit=$5 lic=$6 n=0
  while IFS= read -r -d '' f; do
    local rel=${f#"$work/$sub"/}
    mkdir -p "$(dirname "$OUT/$dest/$rel")"
    cp "$f" "$OUT/$dest/$rel"
    provenance "$dest/$rel" "https://github.com/$repo/blob/$commit/$sub/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work/$sub" -type f -print0)
  echo "  $repo: $n files -> $dest"
}

# ------------------------------------------------------------------------------- sections

KHRONOS_COMMIT=edc7c9e67c639d230715049ee31f9a96a6babbbe
KHRONOS_MODELS=(
  Box BoxTextured BoxVertexColors BoxInterleaved Duck Avocado DamagedHelmet WaterBottle
  BoomBox Lantern FlightHelmet Sponza 2CylinderEngine Buggy ReciprocatingSaw GearboxAssy
  MetalRoughSpheres TextureCoordinateTest OrientationTest VertexColorTest
  NormalTangentMirrorTest MultiUVTest SimpleMeshes SimpleSparseAccessor Triangle
  TriangleWithoutIndices AlphaBlendModeTest Fox CesiumMilkTruck CesiumMan BrainStem Cube
  AntiqueCamera Suzanne
)

src_khronos() {      # Khronos glTF-Sample-Assets (each model's own licence; see its README)
  local pats=()
  for m in "${KHRONOS_MODELS[@]}"; do
    pats+=("Models/$m/glTF/*" "Models/$m/glTF-Binary/*" "Models/$m/glTF-Embedded/*")
  done
  local work
  work=$(sparse KhronosGroup/glTF-Sample-Assets "$KHRONOS_COMMIT" "${pats[@]}")
  copy_tree "$work" Models khronos KhronosGroup/glTF-Sample-Assets "$KHRONOS_COMMIT" \
    "per model (CC0 / CC-BY 4.0 / other; Models/<name>/LICENSE.md)"
  rm -rf "$work"
}

TRIMESH_COMMIT=dc8490c825c382d2d58b5bd87ecacc74790ead27

src_trimesh() {      # trimesh's test models (MIT)
  local work
  work=$(sparse mikedh/trimesh "$TRIMESH_COMMIT" \
    '/models/*.stl' '/models/*.STL' '/models/*.obj' '/models/*.OBJ' '/models/*.mtl' \
    '/models/*.ply' '/models/*.3mf' '/models/*.3MF' '/models/*.glb' '/models/*.gltf' \
    '/models/*.jpg' '/models/*.png' '/models/*.bin')
  copy_tree "$work" models trimesh mikedh/trimesh "$TRIMESH_COMMIT" MIT
  rm -rf "$work"
}

src_oracle() {       # trimesh's counts and bounds (skipped without trimesh)
  if ! "$PYTHON" -I -c "import trimesh" 2>/dev/null; then
    echo "  $PYTHON lacks trimesh; no oracle (see the header for a venv)"
    return
  fi
  "$PYTHON" -I "$ROOT/crates/formats/specular-mesh/gen/oracle.py" "$OUT"
}

main() {
  if (( $# == 0 )); then
    set -- src_khronos src_trimesh src_oracle
    rm -f "$OUT/.provenance.tsv"
  fi
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "unknown section $s"
    echo "$s"
    "$s"
  done
  rm -rf "$SCRATCH"
}

main "$@"
