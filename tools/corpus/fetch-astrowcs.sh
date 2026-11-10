#!/usr/bin/env bash
# Fetch and generate the astronomy WCS / SER / one-shot-colour test corpus into
# tools/corpus/astrowcs/ (gitignored: nothing here is committed). The tests read it in place
# from SPECULAR_ASTROWCS_CORPUS (a worktree has no corpus of its own), else from
# tools/corpus/astrowcs, and skip when it is absent.
#
# Usage: tools/corpus/fetch-astrowcs.sh [SECTION...]
#   SECTION is a function name below (e.g. src_wcs). Default: src_wcs src_wcs_oracle src_ser
#   src_cfa; src_big (timing files) only when named.
#   OUT defaults to $SPECULAR_ASTROWCS_CORPUS, else tools/corpus/astrowcs.
#   PYTHON (default python3) must have numpy and astropy. A venv under target/ is fine:
#   python3 -m venv target/astrowcs/venv && target/astrowcs/venv/bin/pip install
#   --no-cache-dir astropy colour-demosaicing; then PYTHON=target/astrowcs/venv/bin/python.
#
# What it covers (about 6 MB; src_big adds a 60 MP CFA FITS of 120 MB and a 4 GB SER that is
# sparse on disk, a few MB actually written):
#   wcs/            astropy's WCS test headers and files at v7.1.0 (every Paper II projection
#                   map, spectral axes, SIP, TPV, ZPN, D2IM / CPDIS lookup tables, odd headers)
#   wcs/expected.tsv   astropy (wcslib) pixel -> world over a grid of every file (the oracle,
#                   crates/formats/specular-fits/gen/wcs_oracle.py corpus)
#   ser/            generated SER captures: mono / Bayer / RGB / BGR, 8 and 16 bit, both byte
#                   orders and the LittleEndian flag quirk, timestamps, a cut file
#   cfa/            generated one-shot-colour FITS: BAYERPAT / COLORTYP, offsets, ROWORDER
#   big/            timing files: big/capture-4g.ser, big/cfa-60mp.fits
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Scratch space is target/agent-corpus/astrowcs-fetch (deleted afterwards). Stops if free space
# on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_ASTROWCS_CORPUS:-$ROOT/tools/corpus/astrowcs}
SCRATCH="$ROOT/target/agent-corpus/astrowcs-fetch"
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
export GIT_LFS_SKIP_SMUDGE=1 GIT_TERMINAL_PROMPT=0
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-astrowcs: $*" >&2; exit 1; }

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
# Sparse, blobless, depth-1 fetch of PATH... at COMMIT; copies the header and FITS files to
# $OUT/DEST/<file name>.
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
  mkdir -p "$OUT/$dest"
  while IFS= read -r -d '' f; do
    local rel=${f#"$work"/}
    cp "$f" "$OUT/$dest/$(basename "$rel")"
    provenance "$dest/$(basename "$rel")" "https://github.com/$repo/blob/$commit/$rel" "$lic"
    n=$((n + 1))
  done < <(find "$work" -path "$work/.git" -prune -o -type f \
             \( -iname '*.hdr' -o -iname '*.fits' -o -iname '*.fits.gz' -o -iname '*.fit' \) -print0)
  rm -rf "$work"
  echo "  $repo: $n files -> $dest"
}

need_python() {
  "$PYTHON" -I -c "import $1" 2>/dev/null || die "$PYTHON lacks '$1' (see the header for a venv)"
}

# ------------------------------------------------------------------------------- sections

src_wcs() {          # astropy's WCS test data, v7.1.0 (BSD-3-Clause)
  git_files astropy/astropy 8f2535d08a6e496134c6d3f63d5fcae67ded35dc wcs BSD-3-Clause \
    'astropy/wcs/tests/data/*'
}

src_wcs_oracle() {   # astropy pixel -> world over every file under wcs/
  need_python astropy
  "$PYTHON" -I "$ROOT/crates/formats/specular-fits/gen/wcs_oracle.py" corpus "$OUT/wcs"
}

src_ser() {          # generated SER captures
  need_python numpy
  check_space
  "$PYTHON" -I "$ROOT/crates/formats/specular-ser/gen/make_ser.py" samples "$OUT/ser"
  provenance ser "crates/formats/specular-ser/gen/make_ser.py" "generated"
  echo "  ser: $(find "$OUT/ser" -name '*.ser' | wc -l) files -> ser"
}

src_cfa() {          # generated one-shot-colour FITS
  need_python astropy
  check_space
  "$PYTHON" -I "$ROOT/crates/formats/specular-ser/gen/make_ser.py" cfa "$OUT/cfa"
  provenance cfa "crates/formats/specular-ser/gen/make_ser.py" "generated"
  echo "  cfa: $(find "$OUT/cfa" -name '*.fits' | wc -l) files -> cfa"
}

src_big() {          # timing files (a sparse 4 GB SER, a 60 MP CFA FITS)
  need_python astropy
  check_space
  "$PYTHON" -I "$ROOT/crates/formats/specular-ser/gen/make_ser.py" big "$OUT/big"
  provenance big "crates/formats/specular-ser/gen/make_ser.py" "generated"
  echo "  big: $(du -sh --apparent-size "$OUT/big" | cut -f1) apparent, $(du -sh "$OUT/big" | cut -f1) on disk"
}

main() {
  if (( $# == 0 )); then
    set -- src_wcs src_wcs_oracle src_ser src_cfa
  fi
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "unknown section $s"
    echo "$s"
    "$s"
  done
  rm -rf "$SCRATCH"
}

main "$@"
