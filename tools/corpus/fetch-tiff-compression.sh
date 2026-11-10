#!/usr/bin/env bash
# Build the TIFF compression corpus in $SPECULAR_LARGE_CORPUS/tiff-compression (default
# tools/corpus/large/tiff-compression; gitignored, nothing here is committed).
#
# Usage: tools/corpus/fetch-tiff-compression.sh [SECTION...]
#   SECTION is a function name below. Default: gdal generate (svs is opt-in: 61 MB).
#
# * gdal: small files from GDAL's autotest data at a pinned tag: LERC, ZSTD, WebP, CCITT G4,
#   16/24/32/64-bit float (incl. MinIsWhite float).
# * generate: synthetic fixtures written by tiff-oracle.py (tifffile + imagecodecs, Pillow
#   for CCITT): ZSTD/LZMA/Deflate with predictors 2 and 3 in both byte orders, LERC v2..v6
#   (all data types, masks, lossy, Deflate/ZSTD wrapped), WebP, JPEG XL, JPEG 2000 (33003,
#   33005, 34712), CCITT G3/G4/RLE and an A0 400 dpi G4 sheet (18720x13244).
# * svs: OpenSlide's Aperio JP2K-33003-1.svs (61 MB, YCbCr JPEG 2000 tiles).
# * oracle: runs tiff-oracle.py compare over everything here with the `dump` example
#   (build it first: cargo build --release -p specular-tiff --example dump).
#
# Needs curl and, for generate/oracle, a python with numpy, tifffile, imagecodecs and Pillow
# ($PYTHON, default target/tiffcomp/venv/bin/python, else python3). Downloads are
# untrusted data: only copied, and read with `python -I`. Stops if free space drops below
# MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
LARGE=${SPECULAR_LARGE_CORPUS:-$ROOT/tools/corpus/large}
OUT=$LARGE/tiff-compression
MIN_FREE_GB=${MIN_FREE_GB:-6}
PYTHON=${PYTHON:-$ROOT/target/tiffcomp/venv/bin/python}
[[ -x $PYTHON ]] || PYTHON=python3
DUMP=${DUMP:-$ROOT/target/release/examples/dump}
mkdir -p "$OUT"

die() { echo "fetch-tiff-compression: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH SOURCE LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# ------------------------------------------------------------------------------- sections

gdal() {
  local tag=v3.9.0 n=0 f
  local base="https://raw.githubusercontent.com/OSGeo/gdal/$tag/autotest/gcore/data"
  check_space
  mkdir -p "$OUT/gdal"
  for f in byte_lerc.tif byte_zstd.tif tif_webp.tif slim_g4.tif float16.tif float24.tif \
           float32.tif float64.tif float32_minwhite.tif; do
    curl -fsSL --retry 3 -o "$OUT/gdal/$f" "$base/$f"
    provenance "gdal/$f" "$base/$f" 'GDAL autotest data (MIT project; test data)'
    n=$((n + 1))
  done
  echo "  gdal: $n files -> gdal"
}

generate() {
  check_space
  mkdir -p "$OUT/gen"
  "$PYTHON" -I "$ROOT/tools/corpus/tiff-oracle.py" generate "$OUT/gen" >/dev/null
  for f in "$OUT"/gen/*.tif; do
    provenance "gen/$(basename "$f")" 'tools/corpus/tiff-oracle.py generate' 'generated'
  done
  echo "  generated: $(ls "$OUT"/gen/*.tif | wc -l) files -> gen"
}

svs() {
  local url=https://openslide.cs.cmu.edu/download/openslide-testdata/Aperio/JP2K-33003-1.svs
  check_space
  mkdir -p "$OUT/svs"
  curl -fsSL --retry 3 -o "$OUT/svs/JP2K-33003-1.svs" "$url"
  provenance svs/JP2K-33003-1.svs "$url" 'OpenSlide test data (CC0 1.0)'
  echo "  svs: JP2K-33003-1.svs"
}

oracle() {
  [[ -x $DUMP ]] || die "no dump binary at $DUMP (set DUMP=...)"
  local files
  mapfile -t files < <(find "$OUT" \( -name '*.tif' -o -name '*.tiff' -o -name '*.svs' \) | sort)
  (( ${#files[@]} )) || die "nothing to compare"
  # The comparison writes its scratch output to ./oracle-out.
  mkdir -p "$ROOT/target/tiff-oracle"
  (cd "$ROOT/target/tiff-oracle" && "$PYTHON" -I "$ROOT/tools/corpus/tiff-oracle.py" compare "$DUMP" "${files[@]}")
}

main() {
  local sections=() s
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,23p' "$0"; exit 0 ;;
      *) sections+=("$a") ;;
    esac
  done
  (( ${#sections[@]} )) || sections=(gdal generate)
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    "$s"
  done
  if [[ -f $OUT/.provenance.tsv ]]; then
    tac "$OUT/.provenance.tsv" | awk -F'\t' '!seen[$1]++' | tac > "$OUT/.provenance.tmp"
    mv "$OUT/.provenance.tmp" "$OUT/.provenance.tsv"
  fi
  du -sh "$OUT"
}
main "$@"
