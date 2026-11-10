#!/usr/bin/env bash
# Build the FLIF test corpus in tools/corpus/editors/flif (gitignored, nothing here is
# committed). The specular-flif oracle test reads it from there and skips when it is absent.
#
# Usage: tools/corpus/fetch-editors-flif.sh [SECTION...]
#   SECTION is a function name below (oracle, photos, samples, generate, encode, truncate,
#   reference). Default: all, in that order.
#
# * oracle: the reference FLIF tool (FLIF-hub/FLIF, pinned commit), built as an oracle-only
#   binary under target/editors/oracle/flif. Needs g++, make, pkg-config and the libpng
#   headers; nothing is installed.
# * photos: three Kodak test photos (r0k.us), sources for real-image encodes.
# * samples: the FLIF samples of sembiance.com's file format collection.
# * generate: synthetic sources (tools/corpus/flif-generate.py, numpy + Pillow from the
#   oracle venv target/editors/venv or PYTHON).
# * encode: every source through the reference encoder with many option sets (interlaced
#   and not, effort, lossy, forced/disabled transforms, predictors, chances, palettes,
#   animations with frame delays, lookback and frame shapes).
# * truncate: cut copies of a few interlaced files (progressive decoding).
# * reference: `flif -d` of every .flif as <name>.pam (animations: <name>-NNN.pam).
#
# Downloads are untrusted data: this script only copies them and hands them to the oracle.
# Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=$ROOT/tools/corpus/editors/flif
# Build outputs live in the main checkout's target/ (also when run from a worktree).
TARGET=${SPECULAR_TARGET:-${ROOT%%/.claude/worktrees/*}/target}
ORACLE_DIR=${ORACLE_DIR:-$TARGET/editors/oracle/flif}
FLIF_COMMIT=0074d6fd095d27ce81346aa3fbe9bab59105053e
FLIF=$ORACLE_DIR/FLIF-$FLIF_COMMIT/src/flif
PYTHON=${PYTHON:-$TARGET/editors/venv/bin/python}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT" "$ORACLE_DIR"

die() { echo "fetch-editors-flif: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH SOURCE LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

oracle() {
  check_space
  if [[ -x $FLIF ]]; then return; fi
  curl -fsSL -o "$ORACLE_DIR/flif-$FLIF_COMMIT.tar.gz" \
    "https://codeload.github.com/FLIF-hub/FLIF/tar.gz/$FLIF_COMMIT"
  tar -xzf "$ORACLE_DIR/flif-$FLIF_COMMIT.tar.gz" -C "$ORACLE_DIR"
  (cd "$ORACLE_DIR/FLIF-$FLIF_COMMIT/src" && TMPDIR=$ORACLE_DIR nice make -j2 flif >/dev/null)
  [[ -x $FLIF ]] || die "oracle build failed"
}

photos() {
  check_space
  mkdir -p "$OUT/photos"
  for n in 03 15 23; do
    local f=$OUT/photos/kodim$n.png
    [[ -s $f ]] || curl -fsSL -o "$f" "https://r0k.us/graphics/kodak/kodak/kodim$n.png"
    provenance "photos/kodim$n.png" "https://r0k.us/graphics/kodak/" "Kodak test image, free for unrestricted use"
  done
}

samples() {
  check_space
  mkdir -p "$OUT/flif"
  for n in 335_b3a_0.flif abydos.flif; do
    local f=$OUT/flif/sembiance-$n
    [[ -s $f ]] || curl -fsSL -o "$f" "https://sembiance.com/fileFormatSamples/image/flif/$n"
    provenance "flif/sembiance-$n" "https://sembiance.com/fileFormatSamples/image/flif/" "sample collection, unknown licence"
  done
}

generate() {
  check_space
  rm -rf "$OUT/src"
  "$PYTHON" -I "$ROOT/tools/corpus/flif-generate.py" "$OUT" "$OUT"/photos/*.png
}

# enc NAME OPTIONS... INPUTS...
enc() {
  local name=$1; shift
  "$FLIF" -e -o "$@" "$OUT/flif/$name.flif" >/dev/null 2>&1 || echo "encode failed: $name"
}

encode() {
  check_space
  mkdir -p "$OUT/flif"
  local s=$OUT/src
  local f base
  for f in "$s"/*.png "$s"/*.ppm "$s"/*.pgm "$s"/*.pam; do
    [[ -e $f ]] || continue
    base=$(basename "${f%.*}")
    enc "$base-N" -N "$f"
    enc "$base-I" -I "$f"
  done
  # Option sets on a few sources.
  for f in grad-rgb8.png grad-rgba8.png photo0.png palette-rgb8.png grad-rgb16.ppm grad-grey8.png photo1-rgba.png; do
    [[ -e $s/$f ]] || continue
    base=$(basename "${f%.*}")
    enc "$base-E0" -E0 "$s/$f"
    enc "$base-E100" -E100 "$s/$f"
    enc "$base-NE100" -N -E100 "$s/$f"
    enc "$base-Q50" -Q50 "$s/$f"
    enc "$base-Q80N" -Q80 -N "$s/$f"
    enc "$base-A" -A "$s/$f"
    enc "$base-AN" -A -N "$s/$f"
    enc "$base-B" -B "$s/$f"
    enc "$base-C" -C "$s/$f"
    enc "$base-Y" -Y "$s/$f"
    enc "$base-W" -W "$s/$f"
    enc "$base-YN" -Y -N "$s/$f"
    enc "$base-X8Z30" -X8 -Z30 "$s/$f"
    enc "$base-X1Z2N" -X1 -Z2 -N "$s/$f"
    enc "$base-G0" -G0 "$s/$f"
    enc "$base-G1" -G1 "$s/$f"
    enc "$base-G2" -G2 "$s/$f"
    enc "$base-GX" -GX "$s/$f"
    enc "$base-P0" -P0 "$s/$f"
    enc "$base-P10000" -P10000 "$s/$f"
    enc "$base-R0" -R0 "$s/$f"
    enc "$base-J" -J "$s/$f"
  done
  for f in grad-rgba8.png photo0-rgba.png palette-rgba8.png grad-rgba16.pam; do
    [[ -e $s/$f ]] || continue
    base=$(basename "${f%.*}")
    enc "$base-K" -K "$s/$f"
    enc "$base-KN" -K -N "$s/$f"
    enc "$base-H0" -H0 "$s/$f"
    enc "$base-H1" -H1 "$s/$f"
    enc "$base-H2" -H2 "$s/$f"
    enc "$base-H1GX" -H1 -GX "$s/$f"
  done
  [[ -e $s/cb-src.png ]] && enc cb-AN -A -N "$s/cb-src.png" && enc cb-AI -A -I "$s/cb-src.png"
  [[ -e $s/meta-src.png ]] && enc meta "$s/meta-src.png"
  # Animations.
  local d
  for d in "$s"/anim-*; do
    [[ -d $d ]] || continue
    base=$(basename "$d")
    enc "$base-N" -N -F40,80,120 "$d"/*.png
    enc "$base-I" -I -F100 "$d"/*.png
    enc "$base-L3" -L3 "$d"/*.png
    enc "$base-L3N" -L3 -N "$d"/*.png
    enc "$base-S" -S "$d"/*.png
    enc "$base-SN" -S -N "$d"/*.png
    enc "$base-L0" -L0 "$d"/*.png
    enc "$base-Q70" -Q70 "$d"/*.png
  done
}

truncate_files() {
  check_space
  local f n size
  for f in grad-rgb8-I photo0-I grad-rgba8-I palette-rgb8-I grad-rgb16-I anim-rgba-I; do
    [[ -e $OUT/flif/$f.flif ]] || continue
    size=$(stat -c %s "$OUT/flif/$f.flif")
    for n in 10 35 70 95; do
      head -c $((size * n / 100)) "$OUT/flif/$f.flif" > "$OUT/flif/$f-cut$n.flif"
    done
  done
}

reference() {
  check_space
  local f
  for f in "$OUT"/flif/*.flif; do
    rm -f "${f%.flif}".pam "${f%.flif}"-[0-9][0-9][0-9].pam "${f%.flif}".pnm "${f%.flif}"-[0-9][0-9][0-9].pnm
    "$FLIF" -d -o "$f" "${f%.flif}.pam" >/dev/null 2>&1 || true
  done
  ls "$OUT"/flif/*.flif | wc -l | xargs echo "files:"
}

if (( $# == 0 )); then set -- oracle photos samples generate encode truncate reference; fi
for s in "$@"; do
  case $s in
    truncate) truncate_files ;;
    *) "$s" ;;
  esac
done
