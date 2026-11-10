#!/usr/bin/env bash
# Fetch the public CGM test corpus into tools/corpus/docs/cgm/ (gitignored: nothing downloaded
# here is committed). The specular-cgm oracle test reads it from SPECULAR_CGM_CORPUS, else from
# tools/corpus/docs/cgm, and skips when it is absent.
#
# Usage: tools/corpus/fetch-cgm.sh
#
# What it covers (about 20 MB: the .cgm files, and the suites' reference PNGs in ref/): the OASIS/CGM Open WebCGM 2.0 and
# 2.1 test suites (static rendering tests of every primitive and attribute, fonts, cell arrays,
# tile arrays, APS structures), the W3C WebCGM example illustrations (real technical
# illustrations from S1000D-style documentation) and the CGM Open WebCGM demo. Licences: the
# test suites are published by OASIS for conformance testing (OASIS IPR policy, freely
# redistributable test materials); the W3C samples under the W3C document licence.
#
# Downloads are untrusted data: this script only unpacks and copies .cgm files, never runs
# anything from them. Scratch space is target/corpus/cgm-fetch (deleted afterwards).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_CGM_CORPUS:-$ROOT/tools/corpus/docs/cgm}
SCRATCH="$ROOT/target/corpus/cgm-fetch"
mkdir -p "$OUT" "$SCRATCH"

die() { echo "fetch-cgm: $*" >&2; exit 1; }

# zip_set NAME URL: downloads URL, keeps every .cgm in it under $OUT/NAME/ (flattened).
zip_set() {
  local name=$1 url=$2 zip="$SCRATCH/$1.zip" dir="$SCRATCH/$1"
  if [[ -d "$OUT/$name" ]] && compgen -G "$OUT/$name/*.cgm" > /dev/null; then
    echo "fetch-cgm: $name present"
    return
  fi
  echo "fetch-cgm: $name <- $url"
  curl -fsSL --retry 3 -o "$zip" "$url" || die "download failed: $url"
  rm -rf "$dir"
  mkdir -p "$dir" "$OUT/$name"
  unzip -q -o "$zip" -d "$dir"
  find "$dir" -type f -iname '*.cgm' -print0 | while IFS= read -r -d '' f; do
    cp -- "$f" "$OUT/$name/$(basename -- "$f")"
  done
  # The test suites' reference renderings (what a conforming viewer shows), for eyeballing.
  find "$dir" -type f -ipath '*/images/*.png' -print0 | while IFS= read -r -d '' f; do
    mkdir -p "$OUT/$name/ref"
    cp -- "$f" "$OUT/$name/ref/$(basename -- "$f")"
  done
  printf '%s\t%s\n' "$name" "$url" >> "$OUT/.provenance.tsv"
}

zip_set webcgm20-ts https://docs.oasis-open.org/webcgm/test-materials/webcgm20ts/webcgm20-ts.zip
zip_set webcgm21-ts https://docs.oasis-open.org/webcgm/test-materials/webcgm21ts/webcgm21-ts.zip
zip_set w3c-illustrations https://www.w3.org/Graphics/WebCGM/2006/samples/example_illustrations.zip
zip_set cgmopen-demo http://www.cgmopen.org/technical/WebCGM-demo.zip

rm -rf "$SCRATCH"
echo "fetch-cgm: $(find "$OUT" -iname '*.cgm' | wc -l) files, $(du -sh "$OUT" | cut -f1)"
