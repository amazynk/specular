#!/bin/sh
# Fetches the SHX oracle (@mlightcad/shx-parser 1.3.5, MIT) under $CARGO_TARGET_DIR/ref/shxp.
# Used only by tools/corpus/shx-oracle.mjs for test comparisons; never shipped or committed.
set -eu
dir="${CARGO_TARGET_DIR:-target}/ref/shxp"
mkdir -p "$dir"
url=https://registry.npmjs.org/@mlightcad/shx-parser/-/shx-parser-1.3.5.tgz
echo "Downloading $url"
curl -fL --progress-bar "$url" -o "$dir/p.tgz"
tar -xzf "$dir/p.tgz" -C "$dir"
echo "Oracle in $dir/package"
