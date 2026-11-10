#!/usr/bin/env bash
# Fetch and generate the WSQ test corpus into tools/corpus/raster/wsq/ (gitignored) with NBIS's
# decoding of every file (the oracle) under expect/.
#
# Usage: tools/corpus/fetch-raster-wsq.sh
#   OUT defaults to $SPECULAR_RASTER_CORPUS_WSQ, else tools/corpus/raster/wsq.
#   PYTHON must have the `wsq` package (NIST NBIS in a Pillow plugin, CeCILL-C), numpy and
#   Pillow; a venv under target/ is fine:
#     python3 -m venv target/raster/venv
#     target/raster/venv/bin/pip install --no-cache-dir wsq numpy pillow
#   `magick` (ImageMagick) supplies the built-in pictures that are encoded.
#   Then: SPECULAR_RASTER_CORPUS_WSQ=tools/corpus/raster/wsq \
#     cargo test -p specular-wsq --test oracle -- --ignored --nocapture
#
# Downloads (pinned commits; test data only, nothing is linked or copied into the code):
#   jnbis-sample.wsq       mhshams/jnbis (Apache-2.0)
#   python-wsq-test.wsq    idemia/python-wsq
#   biomsharp-fp01.wsq     BiomSharp/BiomSharp
# Then about 55 files encoded with NBIS at 0.3 to 4 bits per pixel, odd and even sizes.
# Downloads are untrusted data: this script only stores them; Python runs isolated (-I).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RASTER_CORPUS_WSQ:-$ROOT/tools/corpus/raster/wsq}
PYTHON=${PYTHON:-python3}
MIN_FREE_GB=${MIN_FREE_GB:-8}
JNBIS=4905b61a9be466bbaf16bcff5cbde82e7095531b
PYWSQ=b3c520cdead63017bea080cdfc4b6e13dde21a21
BIOMSHARP=c9af29586fe37252ebe98fd4c964ad3e75e793e2
GH=https://raw.githubusercontent.com

mkdir -p "$OUT/files" "$OUT/expect"
avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
  echo "fetch-raster-wsq: less than ${MIN_FREE_GB} GB free; stopping" >&2
  exit 1
fi

get() {
  local url=$1 dest=$OUT/files/$2
  [[ -s $dest ]] && return 0
  curl -fsSL --retry 3 -o "$dest.part" "$url"
  mv "$dest.part" "$dest"
}

get "$GH/mhshams/jnbis/$JNBIS/src/test/resources/samples/wsq/sample.wsq" jnbis-sample.wsq
get "$GH/idemia/python-wsq/$PYWSQ/wsq/tests/test.wsq" python-wsq-test.wsq
get "$GH/BiomSharp/BiomSharp/$BIOMSHARP/Demos/Sample%20Data/fingerprint_01.wsq" biomsharp-fp01.wsq

"$PYTHON" -I "$ROOT/tools/corpus/raster-wsq-oracle.py" "$OUT"
