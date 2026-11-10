#!/usr/bin/env bash
# Fetch the diffraction-and-volumes corpus (crystallographic CCP4 maps in other axis orders,
# Gaussian cube files) into tools/corpus/diffraction/ (gitignored: nothing here is
# committed), then write the oracle output: crates/formats/specular-em/gen/oracle.py
# (mrcfile, cross-checked with gemmi) for the maps and its cube reader (ASE) for the cubes,
# into OUT/expected/. specular-em's tests/corpus.rs reads SPECULAR_DIFFRACTION_CORPUS (a
# worktree has no corpus of its own), else tools/corpus/diffraction, and skips when absent.
# Detector frames (EDF, SMV, …) have their own corpus: tools/corpus/fetch-xtal.sh.
#
# Usage: tools/corpus/fetch-diffraction.sh [--no-oracle]
#   OUT defaults to $SPECULAR_DIFFRACTION_CORPUS, else tools/corpus/diffraction.
#   PYTHON is a python with numpy, mrcfile, gemmi and ase (default:
#   $CARGO_TARGET_DIR/venv/bin/python).
#
# About 53 MB:
#   maps/   gemmi's test maps (MPL-2.0): 5i55_tiny.ccp4 and iota_yzx.ccp4 (Y, Z, X order),
#           unpacked from .gz
#   cube/   cclib's test cube files (BSD-3-Clause): benzene, CO, NH3 and water densities
#
# Every download is pinned by size and sha256. Downloads are untrusted data: this script
# only copies and unpacks files, never runs anything from them. Stops if free space on the
# target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_DIFFRACTION_CORPUS:-$ROOT/tools/corpus/diffraction}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-${CARGO_TARGET_DIR:-$ROOT/target}/venv/bin/python}
# Downloads keep their names in .dl/ (hidden: the oracle skips it).
DL=$OUT/.dl
mkdir -p "$DL" "$OUT/maps" "$OUT/cube"

die() { echo "fetch-diffraction: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

GEMMI=https://raw.githubusercontent.com/project-gemmi/gemmi/master/tests
CCLIB=https://raw.githubusercontent.com/cclib/cclib/master/test/method

# pinned URL NAME SIZE SHA256: downloads URL once into .dl/NAME and checks size and hash.
pinned() {
  local url=$1 name=$2 size=$3 sha=$4 dest=$DL/$2
  if [[ -f $dest ]] && [[ $(stat -c %s "$dest") == "$size" ]] \
    && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  curl -fsSL --retry 3 --max-filesize $((size + 1)) -o "$dest.part" "$url"
  [[ $(stat -c %s "$dest.part") == "$size" ]] || { rm -f "$dest.part"; die "$name: wrong size $(stat -c %s "$dest.part")"; }
  local got
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  [[ $got == "$sha" ]] || { rm -f "$dest.part"; die "$name: sha256 $got, expected $sha"; }
  mv "$dest.part" "$dest"
}

pinned "$GEMMI/5i55_tiny.ccp4" 5i55_tiny.ccp4 3104 f31e40b834f3b1f835fa1474b543e20b12e9fcd10fdf1a9fda10e2a714dfc3c2
pinned "$GEMMI/iota_yzx.ccp4.gz" iota_yzx.ccp4.gz 162 1ceea29d12e264e355873f740e4d6f500f722f3678a4265e32c531d313288029
cp "$DL/5i55_tiny.ccp4" "$OUT/maps/"
gzip -dc "$DL/iota_yzx.ccp4.gz" > "$OUT/maps/iota_yzx.ccp4"

while IFS=: read -r name size sha; do
  pinned "$CCLIB/$name.cube" "$name.cube" "$size" "$sha"
  cp "$DL/$name.cube" "$OUT/cube/"
done <<'EOF_CUBES'
benzene:28997907:a691992c28a142d68c34f284873a93984c017f5e03798e6a4220b490cf21a4b2
co:8898168:d95fa4f781a67a10c3006f52378ed802319d7490b5dbbbf7b92312355300623f
nh3:3822791:e326aff111b875ae2aac5a25ececf9589f037877ba7346f09364e329a47bb30f
water_fine:11009886:07aeb2deb008e1cc1d5f8614e1350740514871a8b1c461f27f2f667c572345c4
water_mp2:29136:bab1d0df4d6b23ce07a4e13c332929212611860ed7ea2136095c5f8f8e9430ae
EOF_CUBES

if [[ ${1:-} != --no-oracle ]]; then
  [[ -x $PYTHON ]] || die "no python at $PYTHON (see Usage)"
  "$PYTHON" -I "$ROOT/crates/formats/specular-em/gen/oracle.py" "$OUT"
fi
echo "fetch-diffraction: corpus in $OUT"
