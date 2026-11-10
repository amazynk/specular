#!/bin/sh
# DjVu test corpus (public documents, licences as published; gitignored):
#   - the DjVu v3 specification (sndjvu.org): 71 bitonal pages, shared JB2 dictionaries,
#     FGbz palettes, grey IW44 backgrounds, hidden text, an outline
#   - three scanned papers from Bielefeld's 2002 digital-library examples (bundled, JB2)
#   - an Internet Archive book scan (Alice's Adventures in Wonderland, 1866; public domain):
#     colour compound pages, IW44 backgrounds and foregrounds
# Usage: tools/corpus/fetch-djvu.sh [dest] (default tools/corpus/docs/djvu)
set -eu
dest=${1:-"$(dirname "$0")/docs/djvu"}
mkdir -p "$dest"
get() {
  out="$dest/$1"
  [ -s "$out" ] && return 0
  echo "fetching $1"
  curl -fsSL --retry 3 -o "$out.part" "$2"
  mv "$out.part" "$out"
}
get DjVu3Spec.djvu https://www.sndjvu.org/DjVu3Spec.djvu
get feit-thompson.djvu https://www.math.uni-bielefeld.de/~rehmann/DML/Planning/essen/feit-thompson.djvu
get kimmerle-roggenkamp.djvu https://www.math.uni-bielefeld.de/~rehmann/DML/Planning/essen/kimmerle-roggenkamp.djvu
get notbohm.djvu https://www.math.uni-bielefeld.de/~rehmann/DML/Planning/essen/notbohm.djvu
get alice.djvu https://archive.org/download/alicesadventures00carrrich/alicesadventures00carrrich.djvu
ls -la "$dest"
