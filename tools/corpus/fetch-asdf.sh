#!/usr/bin/env bash
# Fetch the ASDF test corpus into tools/corpus/asdf/ (gitignored: nothing here is committed)
# and write asdf-python's reading of it (the oracle) to expected.tsv beside it.
#
# Usage: tools/corpus/fetch-asdf.sh
#   OUT defaults to $SPECULAR_ASDF_CORPUS, else tools/corpus/asdf.
#   PYTHON (default python3) must have asdf, numpy and roman_datamodels for the oracle; a venv
#   under target/ is fine: python3 -m venv target/asdf/venv &&
#   target/asdf/venv/bin/pip install --no-cache-dir asdf roman_datamodels lz4; then
#   PYTHON=target/asdf/venv/bin/python.
#   Then: SPECULAR_ASDF_DIR=tools/corpus/asdf SPECULAR_ASDF_ORACLE=tools/corpus/asdf/expected.tsv
#   cargo test -p specular-asdf --test oracle corpus_matches_asdf_python
#
# What it covers (about 245 MB):
#   standard/  the asdf-standard reference files for every standard version 1.0.0 to 1.6.0 at a
#              pinned commit (BSD-3-Clause): basic, int, float, endian, complex, ascii,
#              structured, compressed (zlib, bzp2), shared and anchored blocks, an exploded
#              (external) array, a streamed block and unicode trees
#   roman/     public simulated Roman WFI products from STScI's Roman Research Nexus tutorial
#              data (stpubdata, public domain US government work): a level-1 ramp (uncal, a
#              3-D uint16 cube with a large tree and GWCS), a segmentation map and its source
#              catalogue; and two CRDS reference files with compressed blocks (a DQ mask and a
#              saturation map)
# Roman calibrated (cal, 336 MB) and mosaic (775 MB) products are larger than this corpus
# allows and are not fetched; the tests build a Roman-like level-2 tree synthetically.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_ASDF_CORPUS:-$ROOT/tools/corpus/asdf}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
STANDARD=786797692c6185cfbacfaf0e1de3b55fb2f33e9e
PUB=https://stpubdata.s3.amazonaws.com/roman
mkdir -p "$OUT/standard" "$OUT/roman"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-asdf: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST LICENCE [optional]: an optional file missing upstream is skipped.
get() {
  local url=$1 dest=$2 lic=$3 optional=${4:-}
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ ! -s "$OUT/$dest" ]]; then
    if ! curl -fsSL --retry 3 -o "$OUT/$dest.part" "$url"; then
      rm -f "$OUT/$dest.part"
      [[ -n $optional ]] && return 0
      die "download failed: $url"
    fi
    mv "$OUT/$dest.part" "$OUT/$dest"
  fi
  printf '%s\t%s\t%s\n' "$dest" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

for v in 1.0.0 1.1.0 1.2.0 1.3.0 1.4.0 1.5.0 1.6.0; do
  for f in anchor ascii basic complex compressed endian exploded exploded0000 float int \
           scalars shared stream structured unicode_bmp unicode_spp; do
    get "https://raw.githubusercontent.com/asdf-format/asdf-standard/$STANDARD/reference_files/$v/$f.asdf" \
        "standard/$v/$f.asdf" "BSD-3-Clause (asdf-standard reference files)" optional
  done
done

lic="public domain (STScI Roman Research Nexus simulated data, stpubdata)"
TUT=$PUB/nexus/soc_simulations/tutorial_data
get "$TUT/r0003201001001001004_0001_wfi03_f106_uncal.asdf" "roman/r0003201001001001004_0001_wfi03_f106_uncal.asdf" "$lic"
get "$TUT/r0003201001001001004_0001_wfi08_f106_segm.asdf" "roman/r0003201001001001004_0001_wfi08_f106_segm.asdf" "$lic"
get "$TUT/r0003201001001001004_0001_wfi06_f106_cat.asdf" "roman/r0003201001001001004_0001_wfi06_f106_cat.asdf" "$lic"
lic="public domain (STScI Roman CRDS reference files, stpubdata)"
get "$PUB/crds/references/roman/roman_wfi_mask_0063.asdf" "roman/roman_wfi_mask_0063.asdf" "$lic"
get "$PUB/crds/references/roman/roman_wfi_saturation_0046.asdf" "roman/roman_wfi_saturation_0046.asdf" "$lic"

# The oracle: asdf-python's reading of every file, paths relative to OUT.
"$PYTHON" -I "$ROOT/crates/formats/specular-asdf/gen/oracle.py" "$OUT" "$OUT/expected.tsv" \
  || die "oracle failed (does PYTHON have asdf, numpy and roman_datamodels?)"
du -sh "$OUT"
