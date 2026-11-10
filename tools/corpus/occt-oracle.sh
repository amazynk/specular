#!/usr/bin/env bash
# Builds the OpenCASCADE oracle (tools/corpus/occt-oracle.cpp) against an OCCT install, writes
# the generated models into the STEP/IGES corpus (gen/) and OCCT's own measurements of every
# model (oracle.tsv), which specular-decode-host's tests/occt.rs compares with.
#
# Usage: OCCT=/path/to/occt/prefix tools/corpus/occt-oracle.sh
#   OCCT    an install prefix with include/opencascade and lib (default: /usr)
#   OUT     the corpus (default: $SPECULAR_OCCT_CORPUS, else tools/corpus/occt; run
#           tools/corpus/fetch-occt.sh first for the downloaded models)
#   BIN     where the oracle program goes (default: target/occt-oracle)
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OCCT=${OCCT:-/usr}
OUT=${OUT:-${SPECULAR_OCCT_CORPUS:-$ROOT/tools/corpus/occt}}
BIN=${BIN:-$ROOT/target/occt-oracle}
LIB=$OCCT/lib
[[ -d $OCCT/lib/x86_64-linux-gnu && ! -e $OCCT/lib/libTKernel.so ]] && LIB=$OCCT/lib/x86_64-linux-gnu

mkdir -p "$(dirname "$BIN")" "$OUT/gen"
nice "${CXX:-c++}" -std=c++17 -O1 -w -I"$OCCT/include/opencascade" \
  "$ROOT/tools/corpus/occt-oracle.cpp" -o "$BIN" -L"$LIB" \
  -lTKDESTEP -lTKDEIGES -lTKDE -lTKXCAF -lTKVCAF -lTKLCAF -lTKCAF -lTKCDF -lTKXSBase -lTKMesh \
  -lTKBO -lTKPrim -lTKShHealing -lTKTopAlgo -lTKGeomAlgo -lTKBRep -lTKGeomBase -lTKG3d \
  -lTKG2d -lTKMath -lTKernel -Wl,--disable-new-dtags,-rpath,"$LIB"

"$BIN" generate "$OUT/gen"
cd "$OUT"
files=()
for f in gen/* occt/* kicad/* pythonocc/*; do
  [[ -f $f ]] && files+=("$f")
done
"$BIN" measure "${files[@]}" > oracle.tsv
echo "occt-oracle: $(wc -l < oracle.tsv) models measured into $OUT/oracle.tsv"
