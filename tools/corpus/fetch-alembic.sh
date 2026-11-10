#!/bin/sh
# Alembic (.abc) test files from OpenUSD's usdAbc tests (Apache-2.0 / modified Apache), with
# the USD baseline one of them was converted to (the oracle for its mesh).
#   tools/corpus/fetch-alembic.sh <folder>
#   SPECULAR_ALEMBIC_CORPUS=<folder> cargo test -p specular-alembic --test corpus -- --ignored --nocapture
set -eu
out=${1:?folder}
mkdir -p "$out"
base=https://raw.githubusercontent.com/PixarAnimationStudios/OpenUSD/release
for p in \
  pxr/usd/bin/usddiff/testenv/testDiffToolVariousUsdFormats/a.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcAlembicData/testasset.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcAlembicData/testusdtimecode.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcCamera/testAbcCamera.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcIndexedGeomArb/hasColorSet.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcIndexedGeomArb/baseline/good_primvars_namespace_after_fix.usda \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcIndexedProperties/indexedTextureCoordinates.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcInstancing/nestedInstancing.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcP_OldEncoding/testUsdAbcP_OldEncoding.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcSDFArguments/testUsdAbcSDFArgumentsFlat.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcSDFArguments/testUsdAbcSDFArgumentsMesh.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcSDFArguments/testUsdAbcSDFArgumentsUV.abc \
  pxr/usd/plugin/usdAbc/testenv/testUsdAbcXformPrimCollapsing/testUsdAbcXformPrimCollapsing.abc \
  pxr/usdValidation/bin/usdchecker/testenv/testUsdChecker/bad/alembic.abc
do
  name=$(basename "$p")
  echo "fetching $name"
  curl -fL --progress-bar -o "$out/$name" "$base/$p"
done
