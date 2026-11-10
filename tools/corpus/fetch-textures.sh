#!/usr/bin/env bash
# Fetch the GPU texture test corpus (KTX2 / KTX / PVR / VTF with ASTC, ETC, EAC, PVRTC, Basis
# Universal ETC1S and UASTC) into tools/corpus/large/texture/ (gitignored: nothing downloaded or
# generated here is committed), then build the reference decoders (astcenc, the Basis Universal
# transcoder) under target/corpus/texture-oracles and write RGBA reference dumps next to
# the files. The specular-texture tests read it from $SPECULAR_LARGE_CORPUS/texture (a worktree
# has no corpus of its own), else tools/corpus/large/texture, and skip when it is absent.
#
# Usage: tools/corpus/fetch-textures.sh [SECTION...]
#   SECTION is a function name below (e.g. src_ktx_software). Default: all sections, in order.
#   OUT defaults to $SPECULAR_LARGE_CORPUS/texture, else tools/corpus/large/texture.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# The oracle builds compile upstream sources at pinned commits (third-party code, built only
# into target/). Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LARGE=${SPECULAR_LARGE_CORPUS:-$ROOT/tools/corpus/large}
OUT="$LARGE/texture"
# Builds go to the main checkout's target/ (shared by its worktrees).
TARGET="$(cd "$(git -C "$ROOT" rev-parse --git-common-dir)/.." && pwd)/target"
SCRATCH="$TARGET/corpus/texture-fetch"
ORACLES="$TARGET/corpus/texture-oracles"
HERE="$ROOT/tools/corpus/texture"
MIN_FREE_GB=${MIN_FREE_GB:-8}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
export OUT ORACLES ROOT MIN_FREE_GB
mkdir -p "$OUT" "$SCRATCH" "$ORACLES"

die() { echo "fetch-textures: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH URL LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# git_files OWNER/REPO COMMIT DEST LICENCE PATH...
# Sparse, blobless, depth-1 fetch of PATH... at COMMIT; copies the files to $OUT/DEST/<repo
# path>. Git LFS pointers are resolved via media.githubusercontent.com.
git_files() {
  local repo=$1 commit=$2 dest=$3 lic=$4; shift 4
  local work="$SCRATCH/${repo//\//_}"
  check_space
  rm -rf "$work"
  git init -q "$work"
  git -C "$work" remote add origin "https://github.com/$repo"
  git -C "$work" config core.sparseCheckout true
  git -C "$work" sparse-checkout set --no-cone "$@"
  git -C "$work" fetch -q --depth 1 --filter=blob:none origin "$commit"
  git -C "$work" -c advice.detachedHead=false checkout -q FETCH_HEAD
  local n=0
  while IFS= read -r -d '' f; do
    local rel=${f#"$work"/}
    mkdir -p "$OUT/$dest/$(dirname "$rel")"
    if head -c 40 "$f" | grep -q '^version https://git-lfs'; then
      curl -fsSL --retry 3 -o "$OUT/$dest/$rel" \
        "https://media.githubusercontent.com/media/$repo/$commit/$rel"
    else
      cp "$f" "$OUT/$dest/$rel"
    fi
    provenance "$dest/$rel" "https://github.com/$repo/blob/$commit/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f -print0)
  rm -rf "$work"
  echo "  $repo: $n files -> $dest"
}

# ------------------------------------------------------------------------------- sections

src_ktx_software() { # KTX-Software test files: KTX2 (ETC1S/BasisLZ, UASTC, ASTC LDR/HDR, ETC2), KTX1
  git_files KhronosGroup/KTX-Software b4b203475bdab723f55ace32ff09a34a7a997f95 ktx-software \
    'Apache-2.0 (KTX-Software); test images' \
    '/tests/resources/ktx2/' '/tests/resources/ktx/'
}

# Per-format oracle sections live in tools/corpus/texture/*.sh (each defines oracle_* functions
# that build their reference decoder into $ORACLES and write references under $OUT).
for f in "$HERE"/*.sh; do
  [[ -f $f ]] && source "$f"
done

main() {
  local sections=() s
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
      *) sections+=("$a") ;;
    esac
  done
  if (( ${#sections[@]} == 0 )); then
    sections+=($(declare -F | awk '{print $3}' | grep '^src_'))
    sections+=($(declare -F | awk '{print $3}' | grep '^oracle_'))
  fi
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    check_space
    "$s"
  done
  if [[ -f $OUT/.provenance.tsv ]]; then
    tac "$OUT/.provenance.tsv" | awk -F'\t' '!seen[$1]++' | tac > "$OUT/.provenance.tmp"
    mv "$OUT/.provenance.tmp" "$OUT/.provenance.tsv"
  fi
  rm -rf "$SCRATCH"
  du -sh "$OUT"
  df -h "$OUT" | tail -1
}
main "$@"
