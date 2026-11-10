#!/usr/bin/env bash
# Fetch the Paint.NET (.pdn) and CorelDRAW (.cdr) test corpus into
# $SPECULAR_EDITORS_CORPUS (default tools/corpus/editors; gitignored, nothing here is committed).
# The specular-pdn / specular-cdr tests read it from there and skip when it is absent.
# PSP, BPG and FLIF have their own scripts (fetch-editors-psp.sh, -bpg.sh, -flif.sh).
#
# Usage: tools/corpus/fetch-editors.sh [SECTION...]
#   SECTION: pdn_sembiance, pdn_pypdn, pdn_more, pdn_oracle, cdr_sembiance, cdr_more,
#   cdr_oracle. Default: all.
#
# * pdn_sembiance: Paint.NET 3.x/4.x documents from sembiance.com's file format samples.
# * pdn_pypdn: pypdn's test documents (MIT) with the flattened PNGs Paint.NET exported for
#   each blend mode: the blend-mode oracle.
# * pdn_more / cdr_more: further documents from public GitHub repositories (pinned commits).
# * cdr_sembiance: CorelDRAW documents (RIFF v3 to X-versions) from sembiance.com.
#
# Downloads are untrusted data: this script only copies them; nothing from them is executed.
# Stops if free space drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_EDITORS_CORPUS:-$ROOT/tools/corpus/editors}
MIN_FREE_GB=${MIN_FREE_GB:-6}
mkdir -p "$OUT"

die() { echo "fetch-editors: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# provenance RELPATH SOURCE LICENCE
provenance() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$OUT/.provenance.tsv"; }

# get URL DEST_RELPATH LICENCE
get() {
  local url=$1 rel=$2 lic=$3
  check_space
  mkdir -p "$(dirname "$OUT/$rel")"
  if [[ ! -s "$OUT/$rel" ]]; then
    curl -fsSL --retry 3 -m 300 -o "$OUT/$rel.part" "$url" || { echo "  failed: $url" >&2; rm -f "$OUT/$rel.part"; return 0; }
    mv "$OUT/$rel.part" "$OUT/$rel"
    provenance "$rel" "$url" "$lic"
  fi
}

SEMB=https://sembiance.com/fileFormatSamples/image
RAW=https://raw.githubusercontent.com

pdn_sembiance() {
  local f
  for f in HOL_ww1_infantry_diffuse.pdn OutLink.pdn bg_overlay_lightmap%2C1.pdn \
           bg_overlay_lightmap2%252C2.pdn buttonmaker2.pdn cursors.pdn diskBad-250.pdn font.pdn \
           gibdon-moon-symbol.pdn ifilm_logo_header.pdn level03.pdn link.pdn mousenobutton.pdn \
           sky.pdn taskbarextender.pdn ui_ingame2_common.pdn; do
    local name=${f//%252C/_}
    name=${name//%2C/_}
    get "$SEMB/paintDotNetImage/$f" "pdn/$name" "sample collection (unknown)"
  done
}

pdn_pypdn() {
  local c=3481b08b3579daf8ab50ecbd03f14619b9c76c8c f
  for f in FlattenBlendTest.pdn Untitled.pdn Untitled2.pdn Untitled3.pdn oldPDN3510.pdn \
           oldPDN3510.png flattenOpacityTest.png layerVisibleTest1.png layerVisibleTest2.png \
           layerVisibleTest3.png \
           FlattenAdditiveTest.png FlattenColorBurnTest.png FlattenColorDodgeTest.png \
           FlattenDarkenTest.png FlattenDifferenceTest.png FlattenGlowTest.png \
           FlattenLightenTest.png FlattenNegationTest.png FlattenOverlayTest.png \
           FlattenReflectTest.png FlattenScreenTest.png FlattenXORTest.png \
           flattenMultiplyTest.png flattenNormalTest.png flattenNormalTest2.png; do
    get "$RAW/addisonElliott/pypdn/$c/tests/data/$f" "pdn/pypdn/$f" "MIT (pypdn)"
  done
}

pdn_more() { :; }

# pypdn's reading of every layer, the layer oracle (tools/corpus/pdn-oracle.py), in a venv
# under target/editors (pip without a cache).
pdn_oracle() {
  local venv=${PDN_VENV:-$ROOT/target/editors/venv}
  if [[ ! -x "$venv/bin/python" ]]; then
    python3 -m venv "$venv"
    PIP_NO_CACHE_DIR=1 "$venv/bin/pip" install -q pypdn numpy pillow
  fi
  "$venv/bin/python" -I "$ROOT/tools/corpus/pdn-oracle.py" "$OUT/pdn"
}

cdr_sembiance() {
  local f
  for f in 11ARMRD0.CDR 70DIVTR1.CDR ARMYSEAL.CDR ARMYSTF1.CDR ARROW5.CDR BALLET.CDR DALI.CDR \
           DRINKS.CDR FLAG.CDR FOX.CDR MENU.CDR TAKE_ONE.CDR WI.CDR example.cdr test.cdr web_cd.cdr; do
    get "$SEMB/cdr/$f" "cdr/$f" "sample collection (unknown)"
  done
}

# LibreOffice's libcdr fuzzing seed corpus (MPL-2.0 project; files of many CorelDRAW versions).
cdr_more() {
  get "https://dev-www.libreoffice.org/corpus/cdrfuzzer_seed_corpus.zip" \
    "cdr/libreoffice/seed.zip" "LibreOffice fuzzing corpus"
  if [[ -s "$OUT/cdr/libreoffice/seed.zip" ]]; then
    (cd "$OUT/cdr/libreoffice" && unzip -o -q seed.zip -x '*/' && rm -f seed.zip)
  fi
}

# Pillow's reading of the preview each .cdr shows first (tools/corpus/cdr-oracle.py).
cdr_oracle() {
  local venv=${PDN_VENV:-$ROOT/target/editors/venv}
  if [[ ! -x "$venv/bin/python" ]]; then
    python3 -m venv "$venv"
    PIP_NO_CACHE_DIR=1 "$venv/bin/pip" install -q pypdn numpy pillow
  fi
  "$venv/bin/python" -I "$ROOT/tools/corpus/cdr-oracle.py" "$OUT/cdr"
}

sections=("$@")
if (( ${#sections[@]} == 0 )); then
  sections=(pdn_sembiance pdn_pypdn pdn_more pdn_oracle cdr_sembiance cdr_more cdr_oracle)
fi
for s in "${sections[@]}"; do
  declare -F "$s" >/dev/null || die "unknown section $s"
  echo "== $s"
  "$s"
done
echo "corpus in $OUT"
