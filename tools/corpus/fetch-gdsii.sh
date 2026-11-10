#!/usr/bin/env bash
# Fetch the GDSII test corpus into tools/corpus/gdsii/ (gitignored: nothing here is committed)
# and write gdstk's reading of every file (the oracle) beside it under expect/.
#
# Usage: tools/corpus/fetch-gdsii.sh
#   OUT defaults to $SPECULAR_GDS_CORPUS, else tools/corpus/gdsii.
#   PYTHON (default python3) must have gdstk for the oracle; a venv under target/ is fine:
#   python3 -m venv target/gdsii/venv && target/gdsii/venv/bin/pip install
#   --no-cache-dir gdstk; then PYTHON=target/gdsii/venv/bin/python.
#   Then: SPECULAR_GDS_CORPUS=tools/corpus/gdsii/files SPECULAR_GDS_EXPECT=tools/corpus/gdsii/expect
#   cargo test -p specular-gds --test oracle corpus_matches_gdstk -- --ignored --nocapture
#
# What it covers (about 210 MB):
#   klayout/   every .gds / .gds.gz under KLayout's testdata at a pinned commit (GPL-2.0+ project;
#              the layouts are test data only, nothing is linked or copied into the code):
#              odd writers, all element kinds, properties, arrays, big coordinates, broken files
#   sram/      the four OpenRAM SKY130 SRAM macros (VLSIDA/sky130_sram_macros, Apache-2.0)
#   sc_hd/     every SKY130 high-density standard cell (google/skywater-pdk-libs-sky130_fd_sc_hd,
#              Apache-2.0): hundreds of small real libraries
#   caravel/   the example user project and its wrapper (efabless/caravel_user_project,
#              Apache-2.0); user_project_wrapper.gds (55 MB) is the timing file
#   gdstk/     gdstk's proof_lib.gds and an OASIS file (refused) (Boost-1.0)
#
# Downloads are untrusted data: this script only copies and gunzips files, never runs anything
# from them. Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_GDS_CORPUS:-$ROOT/tools/corpus/gdsii}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
KLAYOUT=4d927b797d7e2fc9ab2d8ec29781fa6559851663
SRAM=965df150c754fe2b3f93a0bd1f9883eb114279b2
SC_HD=ac7fb61f06e6470b94e8afdf7c25268f62fbd7b1
CARAVEL=f534826abc72af4f9d467446b8b82c9471df68da
GDSTK=c5d1e434e60aaa2695f6a6f8d242255ea56240dd
F="$OUT/files"
mkdir -p "$F" "$OUT/expect" "$OUT/lists"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-gdsii: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE (a .gz URL is stored gunzipped under DEST)
get() {
  local url=$1 dest=$2 lic=$3
  check_space
  mkdir -p "$(dirname "$F/$dest")"
  if [[ ! -s "$F/$dest" ]]; then
    curl -fsSL --retry 3 -o "$F/$dest.part" "$url" || die "download failed: $url"
    # Some .gds.gz files are stored uncompressed.
    if [[ $url == *.gz && $(head -c 2 "$F/$dest.part" | od -An -tx1 | tr -d ' ') == 1f8b ]]; then
      gzip -dc < "$F/$dest.part" > "$F/$dest" || { rm -f "$F/$dest"; echo "bad gzip: $url" >&2; }
      rm -f "$F/$dest.part"
    else
      mv "$F/$dest.part" "$F/$dest"
    fi
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

# list REPO SHA PREFIX: the .gds / .gds.gz paths under PREFIX in a pinned tree
list() {
  local json="$OUT/lists/${1//\//_}.json"
  [[ -s $json ]] || curl -fsSL --retry 3 -o "$json" \
    "https://api.github.com/repos/$1/git/trees/$2?recursive=1" || die "listing failed: $1"
  "$PYTHON" -I -c '
import json, sys
for e in json.load(open(sys.argv[1]))["tree"]:
    p = e["path"]
    if e["type"] == "blob" and p.startswith(sys.argv[2]) and p.endswith((".gds", ".gds.gz")):
        print(p)
' "$json" "$3"
}

while read -r p; do
  d=${p#testdata/}; d=${d%.gz}
  get "https://raw.githubusercontent.com/KLayout/klayout/$KLAYOUT/$p" "klayout/$d" \
      "GPL-2.0-or-later (KLayout test data)"
done < <(list KLayout/klayout $KLAYOUT testdata/)

while read -r p; do
  get "https://raw.githubusercontent.com/VLSIDA/sky130_sram_macros/$SRAM/$p" "sram/${p##*/}" \
      "Apache-2.0 (OpenRAM SKY130 macros)"
done < <(list VLSIDA/sky130_sram_macros $SRAM "")

while read -r p; do
  get "https://raw.githubusercontent.com/google/skywater-pdk-libs-sky130_fd_sc_hd/$SC_HD/$p" \
      "sc_hd/${p##*/}" "Apache-2.0 (SkyWater SKY130 sc_hd)"
done < <(list google/skywater-pdk-libs-sky130_fd_sc_hd $SC_HD cells/)

for f in user_proj_example.gds user_project_wrapper.gds; do
  get "https://raw.githubusercontent.com/efabless/caravel_user_project/$CARAVEL/gds/$f" \
      "caravel/$f" "Apache-2.0 (efabless caravel_user_project)"
done
get "https://raw.githubusercontent.com/heitzmann/gdstk/$GDSTK/tests/proof_lib.gds" \
    "gdstk/proof_lib.gds" "BSL-1.0 (gdstk test data)"
get "https://raw.githubusercontent.com/heitzmann/gdstk/$GDSTK/tests/min_length_path.oas" \
    "gdstk/min_length_path.oas" "BSL-1.0 (gdstk test data)"

# The oracle: gdstk's reading of each file; files gdstk refuses get no .expect.
ORACLE="$ROOT/crates/formats/specular-gds/tests/oracle.py"
(cd "$F" && find . -name '*.gds' | sort) | while read -r p; do
  p=${p#./}
  e="$OUT/expect/${p%.gds}.expect"
  [[ -s $e ]] && continue
  mkdir -p "$(dirname "$e")"
  timeout 600 "$PYTHON" -I "$ORACLE" "$F/$p" "$e" 2>/dev/null || { rm -f "$e"; echo "gdstk refused: $p"; }
done
du -sh "$OUT"
