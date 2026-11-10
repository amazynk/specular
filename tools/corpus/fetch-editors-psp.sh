#!/usr/bin/env bash
# Build the PaintShop Pro test corpus in tools/corpus/editors/psp (gitignored, nothing here is
# committed). The specular-psp corpus test reads it from there (or $SPECULAR_PSP_CORPUS) and
# skips when it is absent.
#
# Usage: tools/corpus/fetch-editors-psp.sh [SECTION...]
#   SECTION is a function name below (src_sembiance, src_graphicex, oracle). Default: all.
#
# * src_sembiance: PSP 5 to PaintShop Pro 2020 files from sembiance.com's file format samples
#   (.psp, .pspimage, a .tub picture tube).
# * src_graphicex: Mike Lischke's GraphicEx test suite (MPL-1.1): 1/4/8-bit palette, grey,
#   24-bit, none/RLE/LZ77, picture tubes (PSP 5 to 7).
# * oracle: GIMP 3's PSP importer writes `<file>.gref` next to each file (tools/corpus/
#   psp-oracle.py): the flattened image and every layer as RGBA8. GIMP refuses some files
#   (noted in its output); those have no reference.
#
# Needs curl and gimp-console 3.x with Python. GIMP runs headless with a private HOME / XDG /
# GEGL swap under target/editors/gimp-psp. Downloads are untrusted data: this script only
# copies them and opens them in GIMP; nothing from them is executed. Stops if free space drops
# below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_PSP_CORPUS:-$ROOT/tools/corpus/editors/psp}
GIMPENV="$ROOT/target/editors/gimp-psp"
GIMP=${GIMP:-$(command -v gimp-console-3.2 || command -v gimp-console-3.0 || command -v gimp-console || true)}
MIN_FREE_GB=${MIN_FREE_GB:-6}
mkdir -p "$OUT"

die() { echo "fetch-editors-psp: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH SOURCE LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# get URL DEST LICENCE
get() {
  local url=$1 dest=$2 lic=$3
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ ! -s $OUT/$dest ]]; then
    curl -fsSL --retry 3 -m 300 -o "$OUT/$dest" "$url" || { echo "  failed: $url"; rm -f "$OUT/$dest"; return 0; }
  fi
  provenance "$dest" "$url" "$lic"
}

src_sembiance() {
  check_space
  local base=https://sembiance.com/fileFormatSamples/image/paintShopPro f
  for f in example.psp example.pspimage hero.pspimage runabout.pspimage sunset06.psp \
           sunset09.psp template.psp zaxxonship.psp; do
    get "$base/$f" "$f" 'sembiance.com file format samples (redistributed as found)'
  done
  get "$base/Dog%20in%20Heart%20Balloon%20by%20Lux.tub" Dog_in_Heart_Balloon_by_Lux.tub \
    'sembiance.com file format samples (redistributed as found)'
}

src_graphicex() {
  check_space
  local commit=9c1c367a82f8f6a0af8afe9699ae39b7861ddb49 p
  local base="https://raw.githubusercontent.com/mike-lischke/GraphicEx/$commit/TestSuite%20original"
  local files=(
    "PSP/16 colors/4 Bit (RLE) RGB 4.psp"
    "PSP/16 colors/4 Bit (uncompressed) RGB 4.psp"
    "PSP/256 Colors/8 Bit (RLE) RGB 4.psp"
    "PSP/256 Colors/8 Bit (no compression) RGB 3.psp"
    "PSP/256 Colors/Grayscale (uncompressed) RGB 4.psp"
    "PSP/Black&White/1Bit (LZ77) RGB 4.psp"
    "PSP/Black&White/1Bit (RLE) RGB 4.psp"
    "PSP/Black&White/1Bit (uncompressed) RGB 4.psp"
    "PSP/Mask & Tubes/3D Gold.tub"
    "PSP/Mask & Tubes/3D Green.tub"
    "PSP/Mask & Tubes/3D Orange.tub"
    "PSP/Mask & Tubes/Flies.tub"
    "PSP/Mask & Tubes/Garden Hose.tub"
    "PSP/True color/Full (LZ77) RGB.psp"
    "PSP/True color/Full (RLE) RGB.psp"
    "PSP/True color/Full (no compression) RGB 3.psp"
    "PSP/True color/Full (no compression) RGB.psp"
    "!Problems!/Chun.psp"
    "!Problems!/Chun2.psp"
    "!Problems!/PSP/download2.psp"
    "!Problems!/PSP/download4.psp"
    "!Problems!/PSP/download6.psp"
    "!Problems!/Timber wolf.psp"
  )
  for p in "${files[@]}"; do
    local url dest
    url="$base/$(printf '%s' "$p" | sed -e 's/ /%20/g' -e 's/&/%26/g' -e 's/!/%21/g')"
    dest="graphicex/$(printf '%s' "$p" | tr ' /&!' '__+_' | sed 's/^_*//')"
    get "$url" "$dest" 'MPL-1.1 (GraphicEx test suite)'
  done
}

oracle() {
  check_space
  [[ -n $GIMP ]] || die "gimp-console not found (set GIMP=)"
  local list="$GIMPENV/list.txt"
  mkdir -p "$GIMPENV"/{home,config,data,cache,run}
  chmod 700 "$GIMPENV/run"
  find "$OUT" -type f \( -iname '*.psp' -o -iname '*.pspimage' -o -iname '*.tub' \) \
    | sort > "$list"
  env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
    HOME="$GIMPENV/home" XDG_CONFIG_HOME="$GIMPENV/config" XDG_DATA_HOME="$GIMPENV/data" \
    XDG_CACHE_HOME="$GIMPENV/cache" XDG_RUNTIME_DIR="$GIMPENV/run" TMPDIR="$GIMPENV" \
    GEGL_SWAP="$GIMPENV/cache" PSP_LIST="$list" \
    timeout 3600 "$GIMP" -i --batch-interpreter python-fu-eval \
    -b "exec(open('$ROOT/tools/corpus/psp-oracle.py').read())" --quit 2>&1 |
    grep -E '^psp-oracle' || true
}

main() {
  local sections=("$@")
  if (( ${#sections[@]} == 0 )); then
    sections=(src_sembiance src_graphicex oracle)
  fi
  local s
  for s in "${sections[@]}"; do
    declare -F "$s" >/dev/null || die "unknown section $s"
    echo "== $s"
    "$s"
  done
  if [[ -f $OUT/.provenance.tsv ]]; then
    awk -F'\t' '!seen[$1]++' "$OUT/.provenance.tsv" > "$OUT/.provenance.tmp"
    mv "$OUT/.provenance.tmp" "$OUT/.provenance.tsv"
  fi
  du -sh "$OUT"
}
main "$@"
