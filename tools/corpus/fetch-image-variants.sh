#!/usr/bin/env bash
# Fetch or build the corpus of rarer image and RAW variants into $OUT (default
# $CARGO_TARGET_DIR/corpus, else target/corpus; gitignored, nothing here is committed).
#
# Usage: tools/corpus/fetch-image-variants.sh [SECTION...]   (default: every section)
#
# * tiff: libtiff's test pictures (pics-3.8.0: ThunderScan, LogL/LogLuv, subsampled YCbCr,
#   old-style LZW, 2-32-bit depths) into tiff-variants/libtiffpic, and the generated layouts
#   of tiff-oracle.py variants (17/20/24/31/64-bit and signed integers, YCbCr 1x1..4x4,
#   old-style LZW, LogL/LogLuv through libtiff, LERC 6 lossless float) into
#   tiff-variants/gen. Oracle: crates/formats/specular-tiff/tests/oracle.rs.
# * iff: Amiga pictures with colour-cycling ranges (TwelveMonkeys' IFF test files) into iff.
#
# The RAW variants (SuperCCD SR, GPR, IIQ in .TIF, monochrome DNG) are in
# tools/corpus/fetch-rawrest.sh.
#
# Needs curl and, for generated files, a python with numpy, tifffile, imagecodecs and Pillow
# ($PYTHON, default $CARGO_TARGET_DIR/venv/bin/python, else python3). Downloads are untrusted
# data: only stored and unpacked, read with `python -I`. Stops if free space drops below
# MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
OUT=${OUT:-$TARGET/corpus}
PYTHON=${PYTHON:-$TARGET/venv/bin/python}
[[ -x $PYTHON ]] || PYTHON=python3
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-image-variants: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance DIR RELPATH SOURCE LICENCE
provenance() { printf '%s\t%s\t%s\n' "$2" "$3" "$4" >> "$1/.provenance.tsv"; }

# fetch URL DEST (skipped when DEST exists)
fetch() {
  [[ -s $2 ]] && return 0
  mkdir -p "$(dirname "$2")"
  curl -fsSL --retry 3 -o "$2.part" "$1" && mv "$2.part" "$2"
}

tiff() {
  local dir=$OUT/tiff-variants url=https://download.osgeo.org/libtiff/pics-3.8.0.tar.gz
  check_space
  mkdir -p "$dir/libtiffpic" "$dir/gen" "$OUT/dl"
  fetch "$url" "$OUT/dl/libtiff-pics-3.8.0.tar.gz"
  tar -xzf "$OUT/dl/libtiff-pics-3.8.0.tar.gz" -C "$dir/libtiffpic" --strip-components=1 \
    --wildcards --no-anchored '*.tif' --exclude='CVS'
  # The depth pictures sit in a subfolder: flatten.
  if [[ -d $dir/libtiffpic/depth ]]; then
    mv "$dir"/libtiffpic/depth/*.tif "$dir/libtiffpic/" && rmdir "$dir/libtiffpic/depth"
  fi
  provenance "$dir" libtiffpic "$url" 'libtiff test pictures (libtiff licence; per-picture notes in its README)'
  "$PYTHON" -I "$ROOT/tools/corpus/tiff-oracle.py" variants "$dir/gen" >/dev/null
  provenance "$dir" gen 'tools/corpus/tiff-oracle.py variants' generated
  echo "  tiff: $(ls "$dir/libtiffpic" | wc -l) libtiff pictures, $(ls "$dir/gen" | wc -l) generated"
}

# Deluxe Paint pictures with colour cycling (CRNG, DRNG) from TwelveMonkeys' IFF test files
# (fair use for testing, see their fair-use.txt), pinned by commit and SHA-256. Test:
# crates/formats/specular-iff/tests/cycling.rs.
iff() {
  local dir=$OUT/iff commit=73ad024833ed74618b81f44c95614845defc1282 e sha name url
  local base="https://raw.githubusercontent.com/haraldk/TwelveMonkeys/$commit/imageio/imageio-iff/src/test/resources/iff"
  check_space
  mkdir -p "$dir"
  for e in \
    'd98eb4424f5878c13befa96c8e37b4eb52b4a02397e0dec2d01fa7dc57dec211 GoldPorsche.iff' \
    'b376e4baf70a42643b29b249885a58e6704fdb328f1baddec37ca43014922d0a Lion.iff' \
    '1a2ced27a597f3a9e157799c94228d259ce5ecf5838336f8485c5ec097d5a40b Bryce.iff' \
    '6712b2603a6a66cbafd3cde121a6c0bd1b86af35db1fc40dc3d91731744c4c5f IKKEGOD.iff' \
    '467881c00f2e6c72e52270a56dbca366fb93d81935ae2d56e2f5240ccde1af97 ASH.PBM'; do
    sha=${e%% *}
    name=${e#* }
    url="$base/$name"
    fetch "$url" "$dir/$name"
    echo "$sha  $dir/$name" | sha256sum -c --quiet - || die "checksum mismatch: $name"
    provenance "$dir" "$name" "$url" 'TwelveMonkeys test data (fair use for testing)'
  done
  echo "  iff: $(ls "$dir" | wc -l) files"
}

SECTIONS=("$@")
[[ ${#SECTIONS[@]} -gt 0 ]] || SECTIONS=(tiff iff)
for s in "${SECTIONS[@]}"; do
  declare -F "$s" >/dev/null || die "unknown section $s"
  "$s"
done
