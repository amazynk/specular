#!/usr/bin/env bash
# Fetch the TwelveMonkeys ImageIO jars (BSD-3-Clause) used as a test oracle for PICT (QuickDraw
# vector drawing, QuickTime images) and IFF (ILBM, PBM, DEEP, RGB8, RGBN): never linked, only
# run by tools/corpus/twelvemonkeys-oracle.java from the oracle tests.
#
# Usage: tools/corpus/fetch-twelvemonkeys.sh [DIR]
#   DIR defaults to $CARGO_TARGET_DIR/twelvemonkeys (or target/twelvemonkeys).
set -euo pipefail
dir=${1:-${CARGO_TARGET_DIR:-target}/twelvemonkeys}
ver=3.12.0
mkdir -p "$dir"
base=https://repo1.maven.org/maven2/com/twelvemonkeys
for j in common/common-lang common/common-io common/common-image imageio/imageio-core \
  imageio/imageio-metadata imageio/imageio-pict imageio/imageio-iff imageio/imageio-tiff \
  imageio/imageio-bmp; do
  name=${j#*/}
  out="$dir/$name-$ver.jar"
  if [ ! -s "$out" ]; then
    echo "fetching $name $ver"
    curl -fL --retry 3 --progress-bar -o "$out.part" "$base/$j/$ver/$name-$ver.jar"
    mv "$out.part" "$out"
  fi
done
echo "TwelveMonkeys $ver in $dir"
