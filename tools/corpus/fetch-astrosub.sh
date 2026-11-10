#!/usr/bin/env bash
# Fetch public star-field FITS for the subframe-metrics oracle into OUT (default
# $CARGO_TARGET_DIR/corpus/astrosub, else target/corpus/astrosub; gitignored, nothing here is
# committed). About 10 MB.
#
#   M6707HH.fits    real CCD image of M67 (photutils-datasets, BSD-3-Clause)
#   hst_wfc3ir_f160w_simulated_starfield.fits   simulated HST WFC3/IR field (same)
#
# The oracle: crates/specular-astro/gen/oracle.py (photutils + astropy in a venv under
# CARGO_TARGET_DIR), run against the TSV that `cargo test -p specular-astro --test oracle --
# --ignored` writes. Downloads are data only: nothing from them is run.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${OUT:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/astrosub}
BASE=https://raw.githubusercontent.com/astropy/photutils-datasets/main/data
mkdir -p "$OUT"
for f in M6707HH.fits hst_wfc3ir_f160w_simulated_starfield.fits; do
  if [ ! -s "$OUT/$f" ]; then
    curl -sfL --max-time 300 -o "$OUT/$f.part" "$BASE/$f"
    mv "$OUT/$f.part" "$OUT/$f"
  fi
  printf '%s\t%s\t%s\n' "$f" "$BASE/$f" "BSD-3-Clause" >> "$OUT/.provenance.tsv"
done
ls -l "$OUT"
