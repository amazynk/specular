#!/usr/bin/env bash
# Fetch the JBIG2 test corpus into tools/corpus/raster/jbig2/ (ignored by version control;
# nothing here is committed): the SerenityOS LibGfx JBIG2 test streams (BSD-2-Clause) at a
# pinned commit, about 2 MB. They cover the T.88 Annex H example, every generic template with
# and without TPGDON and adaptive pixels, MMR, refinement (TPGRON, both templates), symbol
# dictionaries (arithmetic, Huffman, custom tables, refinement/aggregation, context reuse,
# uncompressed collective bitmaps), text regions (every corner, transposed, Huffman run codes),
# halftones (MMR, skip, all templates), striping with unknown height, random-access
# organisation and all combination operators.
#
# The oracle is the hayro-jbig2 crate (a dev-dependency, Apache-2.0/MIT), compared pixel by
# pixel in the ignored test (streams it refuses are compared with bitmap.pgm, the picture they
# encode, made from SerenityOS's bitmap.bmp with ImageMagick):
#   SPECULAR_RASTER_CORPUS_JBIG2=tools/corpus/raster/jbig2 \
#   cargo test -p specular-jbig2 --test oracle -- --ignored --nocapture
#
# Downloads are untrusted data: this script only stores them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RASTER_CORPUS_JBIG2:-$ROOT/tools/corpus/raster/jbig2}
SERENITY=6dfb6940a682df4cc734ee3ef7f3be9b030ab3b0
BASE=https://raw.githubusercontent.com/SerenityOS/serenity/$SERENITY/Tests/LibGfx/test-inputs/jbig2
MIN_FREE_GB=${MIN_FREE_GB:-8}

mkdir -p "$OUT"
avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
  echo "fetch-raster-jbig2: less than ${MIN_FREE_GB} GB free; stopping" >&2
  exit 1
fi

NAMES="annex-h bitmap-composite-and-xnor-halftone bitmap-composite-and-xnor-refine
bitmap-composite-and-xnor-text bitmap-composite-and-xnor
bitmap-composite-or-xor-replace-halftone bitmap-composite-or-xor-replace-refine
bitmap-composite-or-xor-replace-text bitmap-composite-or-xor-replace bitmap-customat-tpgdon
bitmap-customat bitmap-halftone-10bpp-mmr bitmap-halftone-10bpp bitmap-halftone-composite
bitmap-halftone-global bitmap-halftone-grid bitmap-halftone-refine bitmap-halftone-skip-dummy
bitmap-halftone-skip-grid-template1 bitmap-halftone-skip-grid-template2
bitmap-halftone-skip-grid-template3 bitmap-halftone-skip-grid bitmap-halftone-template1
bitmap-halftone-template2 bitmap-halftone-template3 bitmap-halftone
bitmap-initially-unknown-size bitmap-mmr bitmap-p32-eof bitmap-randomaccess
bitmap-refine-customat-tpgron bitmap-refine-customat bitmap-refine-lossless
bitmap-refine-page-subrect bitmap-refine-page bitmap-refine-refine
bitmap-refine-template1-tpgron bitmap-refine-template1 bitmap-refine-tpgron bitmap-refine
bitmap-stripe-initially-unknown-height bitmap-stripe-last-implicit
bitmap-stripe-single-no-end-of-stripe bitmap-stripe-single bitmap-stripe
bitmap-symbol-32bit-arithint bitmap-symbol-big-segmentid
bitmap-symbol-context-reuse-huffman-refagg bitmap-symbol-context-reuse-refagg
bitmap-symbol-context-reuse bitmap-symbol-empty bitmap-symbol-global bitmap-symbol-manyrefs
bitmap-symbol-manysyms bitmap-symbol-negative-sbdsoffset bitmap-symbol-refine
bitmap-symbol-symbolrefine-textrefine-export bitmap-symbol-symbolrefine-textrefine
bitmap-symbol-symbolrefineone-customat bitmap-symbol-symbolrefineone-template1
bitmap-symbol-symbolrefineone bitmap-symbol-symbolrefineseveral
bitmap-symbol-symhuff-texthuff-manysyms bitmap-symbol-symhuff-texthuff
bitmap-symbol-symhuff-texthuffB10B13 bitmap-symbol-symhuffB5B3-texthuffB7B9B12
bitmap-symbol-symhuffcustom-texthuffcustom bitmap-symbol-symhuffrefine-textrefine-export
bitmap-symbol-symhuffrefine-textrefine bitmap-symbol-symhuffrefineone
bitmap-symbol-symhuffrefineseveral bitmap-symbol-symhuffuncompressed-texthuff
bitmap-symbol-textbottomleft bitmap-symbol-textbottomlefttranspose
bitmap-symbol-textbottomright bitmap-symbol-textbottomrighttranspose
bitmap-symbol-textcomposite bitmap-symbol-texthuff-runcodes32-34
bitmap-symbol-texthuff-trailingsymbols bitmap-symbol-texthuffrefine
bitmap-symbol-texthuffrefineB15 bitmap-symbol-texthuffrefinecustom
bitmap-symbol-texthuffrefinecustomdims bitmap-symbol-texthuffrefinecustompos-global
bitmap-symbol-texthuffrefinecustompos bitmap-symbol-texthuffrefinecustomposdims
bitmap-symbol-texthuffrefinecustomsize bitmap-symbol-textrefine-customat
bitmap-symbol-textrefine-negative-delta-width bitmap-symbol-textrefine
bitmap-symbol-texttopright bitmap-symbol-texttoprighttranspose bitmap-symbol-texttranspose
bitmap-symbol bitmap-template1-customat-tpgdon bitmap-template1-customat
bitmap-template1-tpgdon bitmap-template1 bitmap-template2-customat-tpgdon
bitmap-template2-customat bitmap-template2-tpgdon bitmap-template2
bitmap-template3-customat-tpgdon bitmap-template3-customat bitmap-template3-tpgdon
bitmap-template3 bitmap-tpgdon bitmap-trailing-7fff-stripped-harder-refine
bitmap-trailing-7fff-stripped-harder bitmap-trailing-7fff-stripped bitmap"

n=0
for name in $NAMES; do
  f=$name.jbig2
  n=$((n + 1))
  [[ -s "$OUT/$f" ]] && continue
  curl -fsSL --retry 3 -o "$OUT/$f.part" "$BASE/$f"
  mv "$OUT/$f.part" "$OUT/$f"
done
# The picture most "bitmap-*" streams encode, as a second oracle for streams hayro refuses.
if [[ ! -s "$OUT/bitmap.pgm" ]]; then
  curl -fsSL --retry 3 -o "$OUT/bitmap.bmp" \
    "https://raw.githubusercontent.com/SerenityOS/serenity/$SERENITY/Tests/LibGfx/test-inputs/bmp/bitmap.bmp"
  magick "$OUT/bitmap.bmp" -colorspace gray -depth 8 "pgm:$OUT/bitmap.pgm"
fi
echo "fetch-raster-jbig2: $n files in $OUT"
