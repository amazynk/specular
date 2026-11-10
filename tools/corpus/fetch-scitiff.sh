#!/usr/bin/env bash
# Fetch the scientific-TIFF test corpus (ImageJ hyperstacks and LUTs, Zeiss LSM, MetaMorph,
# SEM images with vendor tags) into tools/corpus/scitiff/ (gitignored: nothing here is
# committed) and write tifffile's reading of it next to it. The specular-decode scitiff corpus
# test reads it from SPECULAR_SCITIFF_CORPUS (a worktree has no corpus of its own), else
# tools/corpus/scitiff, and skips when it is absent.
#
# Usage: tools/corpus/fetch-scitiff.sh [SECTION...]
#   SECTION is a function name below (e.g. src_imagej). Default: every section, then the
#   oracle (src_oracle).
#   OUT defaults to $SPECULAR_SCITIFF_CORPUS, else tools/corpus/scitiff.
#   PYTHON (default python3) needs numpy and tifffile for the oracle. A venv under target/ is
#   fine:
#     python3 -m venv target/scitiff-venv && target/scitiff-venv/bin/pip install --no-cache-dir \
#       numpy tifffile
#   then PYTHON=target/scitiff-venv/bin/python.
#
# What it covers (about 6 MB):
#   dl/  ImageJ sample images: a 3-channel composite with inverted LUTs, a contiguous
#        composite, a 16-bit RGB composite (HeLa cells), a 27-slice MRI stack, a float image,
#        M51 (16-bit), a Zeiss LSM, a Zeiss SmartSEM image and two LUTs (fire, grays);
#        OME's LEO SEM grating and a MetaMorph time point.
#   expected/  tifffile's planes, axes, pixel sizes and display ranges (scitiff-oracle.py)
#
# Every download is pinned by sha256. Downloads are untrusted data: this script only copies
# and unzips files, never runs anything from them (the oracle reads them with python -I from a
# script outside the corpus). Stops if free space on the target filesystem drops below
# MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_SCITIFF_CORPUS:-$ROOT/tools/corpus/scitiff}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
mkdir -p "$OUT"

die() { echo "fetch-scitiff: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# pinned URL DEST SHA256 LICENCE: downloads once and checks the hash (a mismatch deletes the
# download and stops).
pinned() {
  local url=$1 dest=$OUT/$2 sha=$3 lic=$4
  if [[ -f $dest ]] && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 -o "$dest.part" "$url"
  local got
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "$2: sha256 $got, expected $sha"
  fi
  mv "$dest.part" "$dest"
  provenance "$2" "$url" "$lic"
}

# list BASE_URL DEST_DIR LICENCE < "SHA256 RELPATH" lines.
list() {
  local base=$1 dir=$2 lic=$3 sha rel n=0
  while read -r sha rel; do
    [[ -z $sha || $sha == \#* ]] && continue
    pinned "$base/${rel// /%20}" "$dir/$rel" "$sha" "$lic"
    n=$((n + 1))
  done
  echo "  $n files -> $dir"
}

# unzipped ZIP MEMBER: extracts one member of a downloaded zip next to it (junking paths).
unzipped() {
  local zip=$OUT/dl/$1 member=$2
  [[ -f $OUT/dl/$(basename "$member") ]] && return
  unzip -q -o -j "$zip" "$member" -d "$OUT/dl"
}

need_python() {
  "$PYTHON" -I -c "import $1" 2>/dev/null || die "$PYTHON lacks '$1' (see the header for a venv)"
}

# ------------------------------------------------------------------------------- sections

IMAGEJ=https://imagej.net/images
OME=https://downloads.openmicroscopy.org/images

src_imagej() {       # ImageJ's sample images (imagej.net/images; terms as published there)
  list "$IMAGEJ" dl "ImageJ sample image" <<'LIST'
35b7483c40f0a5646e1ae5a613b639f83e01345d98f72342a74d6f3e376db892 3_channel_inverted_luts.tif
61b295c3d5badd4447e1921d9ca22beeb939eb32ec0bb08fb4d401a9d5608785 Cntrl1.lsm
f2735843a08bfe44d6bb6f7cebe0a1d25edced7da9857f8143dfc4667737b288 Composite.tif
ad28142a0fe5f34446cd534565ff796bb48d4aa30eb612137186b51ef622289c floatProblm.tif
dcadca9903570721d9dddf53323638bed0e406b4cc08909b6fe635b8b98c8062 m51.tif
b909fc28776f2f2757253a739f2c8f1e40db610579ffb42d013d31982b15af15 SmartSEMSample.tif
7a5d278647af53904995f8c9b67b1165d469639ff8b6c64c3db0c7fc746a4f19 fire.lut
8afaca19d2212ec3aeda9c4771ceb084685c76c3b0b654d5cdaf903de8362e0a grays.lut
9d121c922be1dedad61c7a9902be7384a5c1e1d27ff21bd4a74f554b5574e181 hela-cells.zip
3b00a72f910b4d7722e46dce028af6720ead851008167b960ea7e7adbbcbb82c mri-stack.zip
LIST
  unzipped hela-cells.zip hela-cells.tif
  unzipped mri-stack.zip mri-stack.tif
}

src_ome() {          # OME sample data (downloads.openmicroscopy.org; terms as published there)
  pinned "$OME/LEO/dmankus/2160grating%20512.tif" dl/grating512.tif \
    7ea0272677a37c2a282f3a1e27726b325ac3849cac5b05fc59d84734422d9156 "OME sample data"
  pinned "$OME/Metamorph/zenodo-13642395/test_timelapse_20240816_s1_t1.TIF" \
    dl/metamorph_t1.TIF \
    a945f4f03bfb4518c609af0b7fadfb7b3fcd0211233e585a63d4972d291f3595 "CC-BY-4.0 (Zenodo 13642395)"
}

src_oracle() {       # tifffile's reading of every file above
  need_python numpy
  need_python tifffile
  check_space
  "$PYTHON" -I "$ROOT/tools/corpus/scitiff-oracle.py" "$OUT"
}

main() {
  if (( $# == 0 )); then
    set -- src_imagej src_ome src_oracle
  fi
  for s in "$@"; do
    declare -F "$s" > /dev/null || die "unknown section $s"
    echo "$s"
    "$s"
  done
}

main "$@"
