#!/usr/bin/env bash
# Build the GIMP XCF test corpus in $SPECULAR_LARGE_CORPUS/xcf (default tools/corpus/large/xcf;
# gitignored, nothing here is committed). The specular-xcf tests read it from there and skip
# when it is absent.
#
# Usage: tools/corpus/fetch-xcf.sh [SECTION...]
#   SECTION is a function name below (src_kde, src_gimp, generate, filters, compressed,
#   oracle).
#   Default: all, in that order.
#
# * src_kde / src_gimp: XCF files from KDE kimageformats' autotests and GIMP's own test files
#   (old GIMP versions, RLE tiles, indexed, groups).
# * generate: synthetic files written by GIMP itself (tools/corpus/xcf-generate.py): every
#   layer mode, composite modes and spaces, blend spaces, groups, pass-through, masks,
#   offsets, all precisions, grey and indexed, RLE and zlib, a 2400x1600 bench file.
# * filters: GIMP 3 layer effects (non-destructive filters), one file per filter
#   (tools/corpus/xcf-filters.py).
# * compressed: .xcf.gz / .xcf.bz2 / .xcf.xz copies of a few files.
# * oracle: GIMP's own composite of every .xcf as <file>.ref (tools/corpus/xcf-oracle.py),
#   the reference the tests compare against.
#
# Needs git, curl, gzip, bzip2, xz, and gimp-console 3.x with Python and numpy. GIMP runs
# headless with a private HOME / XDG / GEGL swap under target/corpus/xcf-gimp.
# Downloads are untrusted data: this script only copies them and opens them in GIMP; nothing
# from them is executed. Stops if free space drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_LARGE_CORPUS:-$ROOT/tools/corpus/large}/xcf
SCRATCH="$ROOT/target/corpus/xcf-fetch"
GIMPENV="$ROOT/target/corpus/xcf-gimp"
GIMP=${GIMP:-$(command -v gimp-console-3.2 || command -v gimp-console-3.0 || command -v gimp-console || true)}
MIN_FREE_GB=${MIN_FREE_GB:-6}
export GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-xcf: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH SOURCE LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# git_xcf OWNER/REPO COMMIT DEST LICENCE PATH...
# Sparse, blobless, depth-1 fetch of PATH... at COMMIT; copies every *.xcf* file (flattened
# into DEST).
git_xcf() {
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
  mkdir -p "$OUT/$dest"
  local n=0
  while IFS= read -r -d '' f; do
    local rel=${f#"$work"/}
    cp "$f" "$OUT/$dest/$(basename "$f")"
    provenance "$dest/$(basename "$f")" "https://github.com/$repo/blob/$commit/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f \( -name '*.xcf' -o -name '*.xcf.gz' -o -name '*.xcf.bz2' \
             -o -name '*.xcf.xz' \) -print0)
  rm -rf "$work"
  echo "  $repo: $n files -> $dest"
}

# run_gimp SCRIPT [ENV=VALUE...]: GIMP batch Python in the private environment.
run_gimp() {
  local script=$1; shift
  [[ -n $GIMP ]] || die "gimp-console not found (set GIMP=)"
  mkdir -p "$GIMPENV"/{home,config,data,cache,run}
  chmod 700 "$GIMPENV/run"
  env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
    HOME="$GIMPENV/home" XDG_CONFIG_HOME="$GIMPENV/config" XDG_DATA_HOME="$GIMPENV/data" \
    XDG_CACHE_HOME="$GIMPENV/cache" XDG_RUNTIME_DIR="$GIMPENV/run" TMPDIR="$GIMPENV" \
    GEGL_SWAP="$GIMPENV/cache" "$@" \
    timeout 3600 "$GIMP" -i --batch-interpreter python-fu-eval \
    -b "exec(open('$script').read())" --quit 2>&1 |
    grep -E '^xcf-|Error|error:' || true
}

# ------------------------------------------------------------------------------- sections

src_kde() {     # KDE kimageformats read tests: GIMP 2.x files, RLE, indexed, grey, modes
  git_xcf KDE/kimageformats 5dafbe2d2f81e9a17712c63e2e11345af5f2b4d5 kde \
    'LGPL-2.1+ (KDE kimageformats autotests)' '/autotests/read/xcf/'
}
src_gimp() {    # GIMP's own test files (gimp-2-6-file.xcf and friends)
  git_xcf GNOME/gimp f562a3e60244a713a57a1aac83330746a8bdbd2c gimp \
    'GPL-3.0+ (GIMP test files)' '/app/tests/files/'
}
generate() {
  check_space
  mkdir -p "$OUT/gen"
  run_gimp "$ROOT/tools/corpus/xcf-generate.py" XCF_OUT="$OUT/gen"
  local f
  for f in "$OUT"/gen/*.xcf; do
    provenance "gen/$(basename "$f")" "tools/corpus/xcf-generate.py (GIMP $("$GIMP" --version | awk '{print $NF}'))" 'generated'
  done
}
filters() {     # GIMP 3 layer effects (tools/corpus/xcf-filters.py), one file per filter
  check_space
  mkdir -p "$OUT/filters"
  run_gimp "$ROOT/tools/corpus/xcf-filters.py" XCF_OUT="$OUT/filters"
  local f
  for f in "$OUT"/filters/*.xcf; do
    provenance "filters/$(basename "$f")" "tools/corpus/xcf-filters.py (GIMP $("$GIMP" --version | awk '{print $NF}'))" 'generated'
  done
}
compressed() {
  local f
  for f in prec-u8nl-zlib group-nested pass-mask; do
    [[ -f $OUT/gen/$f.xcf ]] || continue
    gzip -9 -c "$OUT/gen/$f.xcf" > "$OUT/gen/$f.xcf.gz"
    bzip2 -9 -c "$OUT/gen/$f.xcf" > "$OUT/gen/$f.xcf.bz2"
    xz -6 -c "$OUT/gen/$f.xcf" > "$OUT/gen/$f.xcf.xz"
  done
}
oracle() {
  check_space
  local list="$SCRATCH/oracle-list.txt"
  find "$OUT" -type f -name '*.xcf' | sort > "$list"
  run_gimp "$ROOT/tools/corpus/xcf-oracle.py" XCF_LIST="$list"
}

main() {
  local sections=() s
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
      *) sections+=("$a") ;;
    esac
  done
  if (( ${#sections[@]} == 0 )); then
    sections=(src_kde src_gimp generate filters compressed oracle)
  fi
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    "$s"
  done
  if [[ -f $OUT/.provenance.tsv ]]; then
    tac "$OUT/.provenance.tsv" | awk -F'\t' '!seen[$1]++' | tac > "$OUT/.provenance.tmp"
    mv "$OUT/.provenance.tmp" "$OUT/.provenance.tsv"
  fi
  rm -rf "$SCRATCH"
  du -sh "$OUT"
}
main "$@"
