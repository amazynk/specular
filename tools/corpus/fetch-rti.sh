#!/usr/bin/env bash
# Fetch the reflectance transformation imaging corpus into tools/corpus/rti/ (gitignored:
# nothing here is committed) and write the independent numpy reading of it (the oracle,
# crates/formats/specular-rti/gen/oracle.py) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-rti.sh
#   OUT defaults to $SPECULAR_RTI_CORPUS, else tools/corpus/rti.
#   PYTHON (default python3) must have numpy and Pillow for the oracle.
#   Then: SPECULAR_RTI_DIR=tools/corpus/rti SPECULAR_RTI_ORACLE=tools/corpus/rti/expected.tsv
#   cargo test -p specular-rti --test oracle corpus_matches_the_numpy_reference
#
# What it covers (about 160 MB):
#   *.ptm          HP Labs' PTM samples (PTM_1.2 LRGB, RGB and JPEG-compressed), from the
#                  Internet Archive's copies of hpl.hp.com/research/ptm/downloads
#   relight_*/     CNR-ISTI Relight's test folders at a pinned commit (GPL-3.0 repository,
#                  data only): PTM in RGB and LRGB, HSH, RBF over 27 planes, bilinear
#                  18 planes and MYCC 10+4+4, and the 2696×2672 bilinear demo (timing file)
#   relight_openlime_ptm/   OpenLIME's PTM example (GPL-3.0 repository, data only)
#   polycytellidae.rti      a 4933×2203 CHI HSH .rti (order 2) from Zenodo record 5027689
#                  (CC0, "The Polycytellidae viewed as Gondwanan Glosselytrodea")
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RTI_CORPUS:-$ROOT/tools/corpus/rti}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
RELIGHT=c9709ddcbd07026e4022c24c5988e5a655b583ed
OPENLIME=640f70d85c324b672d0d56cc69a61eddff35c455
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-rti: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE
get() {
  local url=$1 dest=$2 lic=$3
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ ! -s "$OUT/$dest" ]]; then
    curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url" || die "download failed: $url"
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

# HP Labs PTMs through the Wayback Machine (timestamp, path under the downloads directory).
WB=http://web.archive.org/web
HP=http://www.hpl.hp.com/research/ptm
while read -r ts path; do
  get "$WB/${ts}id_/$HP/$path" "$(basename "$path")" "HP Labs sample"
done <<'EOF'
20190822163140 downloads/cookwood.ptm
20190822163148 downloads/fossil4.ptm
20190822163156 downloads/leaves.ptm
20190822163245 downloads/newspaper.ptm
20190822163257 downloads/pebbles.ptm
20190822163310 downloads/sanddollar1.ptm
20190822163319 downloads/shell2.ptm
20190822163330 downloads/shell3.ptm
20190822163338 downloads/shell5.ptm
20190822163349 downloads/shell6.ptm
20190822163359 downloads/trilobite.ptm
20210920211646 relightdemo/jpview/ptmfiles/almonds512.ptm
20131105081220 relightdemo/jpview/ptmfiles/trilobitelrgb.ptm
EOF

# Relight folders: DIR FILES...
GH=https://raw.githubusercontent.com
relight() {
  local repo=$1 rev=$2 src=$3 dest=$4
  shift 4
  for f in "$@"; do
    get "$GH/$repo/$rev/$src/$f" "$dest/$f" "$repo data"
  done
}
P9="plane_0.jpg plane_1.jpg plane_2.jpg plane_3.jpg plane_4.jpg plane_5.jpg plane_6.jpg plane_7.jpg plane_8.jpg"
P6="plane_0.jpg plane_1.jpg plane_2.jpg plane_3.jpg plane_4.jpg plane_5.jpg"
# shellcheck disable=SC2086
{
  relight cnr-isti-vclab/relight $RELIGHT test/ptm relight_ptm info.json materials.bin $P6
  relight cnr-isti-vclab/relight $RELIGHT test/lptm relight_lptm info.json plane_0.jpg plane_1.jpg plane_2.jpg
  relight cnr-isti-vclab/relight $RELIGHT test/hsh relight_hsh info.json materials.bin $P9
  relight cnr-isti-vclab/relight $RELIGHT test/rbf27 relight_rbf27 info.json materials.bin $P9
  relight cnr-isti-vclab/relight $RELIGHT test/bilinear18 relight_bilinear18 info.json materials.bin $P6
  relight cnr-isti-vclab/relight $RELIGHT test/ycc1044 relight_ycc1044 info.json materials.bin $P6
  relight cnr-isti-vclab/relight $RELIGHT demo relight_demo info.json $P6
  relight cnr-isti-vclab/openlime $OPENLIME dist/assets/rti/ptm relight_openlime_ptm info.json $P6
}

get "https://zenodo.org/api/records/5027689/files/PRE_F_10559b_overview.rti/content" \
  polycytellidae.rti "CC0 (Zenodo 5027689)"

echo "fetch-rti: running the oracle"
"$PYTHON" -I "$ROOT/crates/formats/specular-rti/gen/oracle.py" "$OUT" "$OUT/expected.tsv"
du -sh "$OUT"
