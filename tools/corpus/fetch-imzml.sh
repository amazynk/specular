#!/usr/bin/env bash
# Fetch the imzML (imaging mass spectrometry) test corpus into tools/corpus/imzml/ (gitignored:
# nothing here is committed) and write pyimzML's reading of it (the oracle) to expected.tsv
# beside it.
#
# Usage: tools/corpus/fetch-imzml.sh
#   OUT defaults to $SPECULAR_IMZML_CORPUS, else tools/corpus/imzml.
#   PYTHON (default python3) must have numpy and pyimzML for the oracle; a venv under target/
#   is fine: python3 -m venv target/imzml/venv && target/imzml/venv/bin/pip install
#   --no-cache-dir pyimzml numpy; then PYTHON=target/imzml/venv/bin/python.
#   Then: SPECULAR_IMZML_DIR=tools/corpus/imzml SPECULAR_IMZML_ORACLE=tools/corpus/imzml/expected.tsv
#   cargo test -p specular-imzml --test oracle corpus_matches_pyimzml
#   and SPECULAR_IMZML_DIR=tools/corpus/imzml cargo test -p specular-decode imzml_corpus_timings
#   -- --nocapture for open / TIC / ion image timings.
#
# What it covers (about 300 MB):
#   pyimzml/   the imzML 1.1 example files from ms-imaging.org (continuous and processed, 3 × 3
#              pixels), as kept in pyimzML's tests at a pinned commit (Apache-2.0)
#   kidney/    MALDI-TOF peptides of a mouse kidney section, continuous (Galaxy training data,
#              zenodo 1560646, MIT)
#   glycans/   MALDI-TOF N-glycans of a murine kidney section, control (zenodo 2628280, MIT;
#              the record's "conrol.imzml" is saved as control.imzml to pair with control.ibd)
#   openfish/  MALDI of a 20 µm section at 8 µm pixels, RMS-normalised SCiLS export with a 54 MB
#              XML: the timing file (zenodo 20390362, CC BY 4.0)
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_IMZML_CORPUS:-$ROOT/tools/corpus/imzml}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
PYIMZML=d84f61ee5a3242d72c594f5210c611383bd9764a
mkdir -p "$OUT/pyimzml" "$OUT/kidney" "$OUT/glycans" "$OUT/openfish"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-imzml: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE [MD5]
get() {
  local url=$1 dest=$2 lic=$3 md5=${4:-}
  check_space
  if [[ ! -s "$OUT/$dest" ]]; then
    curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url" || die "download failed: $url"
    if [[ -n $md5 ]]; then
      [[ $(md5sum "$OUT/$dest.part" | cut -d' ' -f1) == "$md5" ]] || die "checksum mismatch: $url"
    fi
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

for f in Example_Continuous.imzML Example_Continuous.ibd Example_Processed.imzML \
         Example_Processed.ibd; do
  get "https://raw.githubusercontent.com/alexandrovteam/pyimzML/$PYIMZML/tests/data/$f" \
      "pyimzml/$f" "Apache-2.0 (pyimzML test data; ms-imaging.org example files)"
done

Z=https://zenodo.org/api/records
get "$Z/1560646/files/mouse_kidney_cut.imzML/content" kidney/mouse_kidney_cut.imzML \
    "MIT (zenodo 1560646)" 327dcb1499a96b06686832577f033445
get "$Z/1560646/files/mouse_kidney_cut.ibd/content" kidney/mouse_kidney_cut.ibd \
    "MIT (zenodo 1560646)" 05a3780d253eff2b27da394d341f799a
get "$Z/2628280/files/conrol.imzml/content" glycans/control.imzml \
    "MIT (zenodo 2628280)" ea72cc1743f8f2b034fc7b991badcd09
get "$Z/2628280/files/control.ibd/content" glycans/control.ibd \
    "MIT (zenodo 2628280)" 36159b018af8e2998fcf0beb8cd61652
get "$Z/20390362/files/a3-8um-root%20mean%20square.imzML/content" "openfish/a3-8um-rms.imzML" \
    "CC BY 4.0 (zenodo 20390362)" 2549a41f2fcb4403015a2bdfbd4fcc69
get "$Z/20390362/files/a3-8um-root%20mean%20square.ibd/content" "openfish/a3-8um-rms.ibd" \
    "CC BY 4.0 (zenodo 20390362)" ad6e30926c1cfbbff1599e8e9812cf34

# The oracle: pyimzML's reading, per directory, into one TSV with paths relative to OUT
# (at most 2000 spectra per file get value rows, 16 values each).
GEN="$ROOT/crates/formats/specular-imzml/gen/oracle.py"
: > "$OUT/expected.tsv"
for d in pyimzml kidney glycans openfish; do
  "$PYTHON" -I "$GEN" "$OUT/$d" "$OUT/$d.tsv" 2000 16
  sed -e '/^#/d' -e "s#^\([a-z]*\)\t#\1\t$d/#" "$OUT/$d.tsv" >> "$OUT/expected.tsv"
  rm -f "$OUT/$d.tsv"
done
du -sh "$OUT"
