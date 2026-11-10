#!/usr/bin/env bash
# Fetch the 3D and CAD test corpus (Gaussian splats, USD/USDZ, COLLADA, 3DS, LightWave, FBX,
# Blender and SketchUp files) into tools/corpus/models/ (gitignored: nothing here is
# committed). The specular-mesh and specular-splat corpus tests read it from
# SPECULAR_MODELS_CORPUS (a worktree has no corpus of its own), else from tools/corpus/models,
# and skip when it is absent.
#
# Usage: tools/corpus/fetch-models.sh [SECTION...]
#   SECTION is a function name below (e.g. src_splats). Default: every section, then the
#   oracles (src_oracle) when PYTHON has them.
#   OUT defaults to $SPECULAR_MODELS_CORPUS, else tools/corpus/models.
#   PYTHON (default python3) needs usd-core, pycollada, assimp-py, plyfile, trimesh and numpy
#   for the oracle only. A venv under target/ is fine:
#     python3 -m venv target/models/venv
#     PIP_NO_CACHE_DIR=1 target/models/venv/bin/pip install numpy trimesh pycollada \
#       plyfile usd-core assimp-py lxml networkx
#   then PYTHON=target/models/venv/bin/python.
#
# What it covers (about 320 MB):
#   splats   antimatter15 .splat scenes, 3DGS PLY (SH degree 3) with the same scene as
#            .splat, Niantic's .spz samples; every file sha256-pinned
#   usd      usd-wg/assets at a pinned commit: USDA, USDC and USDZ test and full assets
#            (references, payloads, variants, instancing, UsdPreviewSurface textures)
#   assimp   assimp's test models at a pinned commit: COLLADA (.dae, .zae), 3DS, LWO2/LWOB,
#            FBX (binary and ASCII, 6.1 to 7.5) and .blend files
#   skp      a SketchUp model with its embedded preview, sha256-pinned
#   oracle/*.tsv  counts, bounds and sample positions from usd-core, pycollada, assimp and
#            plyfile for every file they read (specular-mesh gen/models_oracle.py), and
#            reference splat renderings (specular-splat gen/splat_oracle.py)
#
# Downloads are untrusted data: this script only copies files, never runs anything from them
# (the oracles read them from a script outside the corpus, with python -I). Scratch space is
# target/agent-corpus/models-fetch (deleted afterwards). Stops if free space on the target
# filesystem drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MODELS_CORPUS:-$ROOT/tools/corpus/models}
SCRATCH="$ROOT/target/agent-corpus/models-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-python3}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-models: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# get URL DEST SHA256 LICENCE: downloads one file and checks its hash.
get() {
  local url=$1 dest=$2 sha=$3 lic=$4
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ -f "$OUT/$dest" ]] && echo "$sha  $OUT/$dest" | sha256sum -c --quiet - 2>/dev/null; then
    echo "  $dest (cached)"
  else
    curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url"
    echo "$sha  $OUT/$dest.part" | sha256sum -c --quiet - || die "$dest: sha256 mismatch"
    mv "$OUT/$dest.part" "$OUT/$dest"
    echo "  $dest"
  fi
  provenance "$dest" "$url" "$lic"
}

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

HF=https://huggingface.co

src_splats() {       # public splat scenes (their authors' terms on Hugging Face; dev use only)
  get "$HF/cakewalk/splat-data/resolve/2c9796f59101d77d23f6eee53baa5018c9a6fcdc/plush.splat" \
    splats/plush.splat 80e0ae670ee980f166d9b00045d33710dd77e65ff004f1886ab442220e2c48fa \
    "cakewalk/splat-data (antimatter15 demo scenes)"
  get "$HF/datasets/solaaaa/sample-gaussian-splats/resolve/main/Akutar/aku.splat" \
    splats/aku.splat 5ee6477526a8a8e8ca1c5af7c603019c88e9d2f83b323f262bd9c520515a7af1 \
    "solaaaa/sample-gaussian-splats"
  get "$HF/datasets/solaaaa/sample-gaussian-splats/resolve/main/CL/CL.splat" \
    splats/CL.splat b33cf1ac3141969983d702f1a8625cf382d69a785f577c958ee889bcd5b38e0e \
    "solaaaa/sample-gaussian-splats"
  get "$HF/datasets/solaaaa/sample-gaussian-splats/resolve/main/CL/CL.ply" \
    splats/CL.ply 83cf352350add75ef8b1342e24f312408dfc7e42172a41ae4ffa8231bfbb3683 \
    "solaaaa/sample-gaussian-splats"
  get "$HF/supernowy/gaussian-splat-test/resolve/main/model10/point_cloud.ply" \
    splats/model10.ply 25afff0667c10bff0986c1ef815b0e54b70b40b7081b292e78f82d68284a8d17 \
    "supernowy/gaussian-splat-test"
  # A coloured triangle mesh named .ply beside the splats: must open as a mesh.
  get "$HF/datasets/rcmt/splat-samples/resolve/main/stickered-payphone.ply" \
    splats/not-a-splat-payphone.ply \
    fc8c3286801fe465296f4e78b22b7f950fa81e6457b387f707eec5fe7e53f795 "rcmt/splat-samples"
  get "https://raw.githubusercontent.com/nianticlabs/spz/affd0ecea7fbb4c265ee119475af7ee5b2997482/samples/hornedlizard.spz" \
    splats/hornedlizard.spz a076ca6aeb9f1a8fe52526f4a7c381fe8fffec60c0d7eb64b45532189b64bc6a MIT
}

USDWG_COMMIT=3b75c2dad6a494897557dcca0098257bcf42a8c6

src_usd() {          # usd-wg/assets (Apache-2.0 and per-asset licences; see each README)
  local work
  work=$(sparse usd-wg/assets "$USDWG_COMMIT" \
    '/test_assets/USDZ/**' '/test_assets/foundation/**' '/test_assets/schemaTests/**' \
    '/test_assets/References/**' '/test_assets/TextureCoordinateTest/**' \
    '/test_assets/NormalsTextureBiasAndScale/**' '/test_assets/_common/**' \
    '/docs/PrimvarInterpolation/**' '/full_assets/Teapot/**' '/full_assets/Vehicles/**' \
    '/full_assets/OpenChessSet/**' '/full_assets/ElephantWithMonochord/**' \
    '/full_assets/SubdivisionSurfaces/**' '/full_assets/McUsd/**')
  copy_tree "$work" . usd usd-wg/assets "$USDWG_COMMIT" "Apache-2.0 / per asset"
  rm -rf "$work"
}

ASSIMP_COMMIT=f20e46b3fc1fdf9bc2675fd549d180dda06861f1

src_assimp() {       # assimp's test models (BSD-3 for test/models; per file in models-nonbsd)
  local work
  work=$(sparse assimp/assimp "$ASSIMP_COMMIT" \
    '/test/models/Collada/**' '/test/models/3DS/**' '/test/models/LWO/**' \
    '/test/models/FBX/**' '/test/models/BLEND/**' '/test/models-nonbsd/3DS/**' \
    '/test/models-nonbsd/FBX/**' '/test/models-nonbsd/LWO/**' '/test/models-nonbsd/BLEND/**')
  copy_tree "$work" test assimp assimp/assimp "$ASSIMP_COMMIT" \
    "BSD-3 (models) / per file (models-nonbsd, see its README)"
  rm -rf "$work"
}

src_skp() {          # a SketchUp 2018 model (ITA, RWTH Aachen; Apache-2.0)
  get "https://git.rwth-aachen.de/ita/ITAPropagationPathSim/-/raw/356feee3a98b17e2932f6d775b10a3ff0f0c42ee/tests/CombinatedModel/SketchUpFiles/ThreeBuildings.skp" \
    skp/ThreeBuildings.skp 84342a9df31977d66fb704964386d94069edf6b5e3aa00748d01554182ff8083 "Apache-2.0 (ITAPropagationPathSim)"
}

src_oracle() {       # usd-core, pycollada, assimp and plyfile facts (skipped without them)
  if ! "$PYTHON" -I -c "import numpy" 2>/dev/null; then
    echo "  $PYTHON lacks numpy; no oracle (see the header for a venv)"
    return
  fi
  "$PYTHON" -I "$ROOT/crates/formats/specular-mesh/gen/models_oracle.py" "$OUT"
  "$PYTHON" -I "$ROOT/crates/formats/specular-splat/gen/splat_oracle.py" "$OUT"
}

main() {
  if (( $# == 0 )); then
    set -- src_splats src_usd src_assimp src_skp src_oracle
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
