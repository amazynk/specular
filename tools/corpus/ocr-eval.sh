#!/usr/bin/env bash
# Runs the OCR oracle comparison on the synthetic set (tools/corpus/ocr-synth.py):
# ours (cargo example `ocr`, GPU) and PaddleOCR 3 (paddlepaddle CPU) per script, Hebrew lines
# through our Tesseract reader and Tesseract 5 built from source; then scores both against the
# truth and against each other (tools/corpus/ocr-oracle.py score).
#
# Usage: tools/corpus/ocr-eval.sh
#   OCR_DIR      the safetensors exports (tools/corpus/fetch-ocr.sh), and paddle/ with the
#                Paddle inference exports of the same models, tessdata_best/heb.traineddata
#   SYNTH_DIR    the synthetic set
#   OCR_BIN      the built example (target/.../examples/ocr)
#   PADDLE_PY    python with paddlepaddle + paddleocr (a venv under target/)
#   TESS_PREFIX  Tesseract's install prefix (bin/tesseract, lib/)
#   SIDE         detector long side (default 1920)
#   PY           python with pillow and shapely (default python3)
# Writes OUT_DIR (default SYNTH_DIR/eval): ours.tsv, ref.tsv, ppm/, crops/.
set -euo pipefail
: "${OCR_DIR:?}" "${SYNTH_DIR:?}" "${OCR_BIN:?}" "${PADDLE_PY:?}" "${TESS_PREFIX:?}"
SIDE=${SIDE:-1920}
PY=${PY:-python3}
OUT=${OUT_DIR:-$SYNTH_DIR/eval}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT/ppm" "$OUT/crops"
: > "$OUT/ours.tsv"
: > "$OUT/ref.tsv"

rec_for() {
  case $1 in
    en|latin|zh|ja) echo PP-OCRv6_small_rec ;;
    ko) echo korean_PP-OCRv5_mobile_rec ;;
    ru) echo cyrillic_PP-OCRv5_mobile_rec ;;
    el) echo el_PP-OCRv5_mobile_rec ;;
    ar) echo arabic_PP-OCRv5_mobile_rec ;;
    th) echo th_PP-OCRv5_mobile_rec ;;
    hi) echo devanagari_PP-OCRv5_mobile_rec ;;
    he) echo tess ;;
  esac
}

for dir in "$SYNTH_DIR"/*/; do
  lang=$(basename "$dir")
  [[ -f "$dir/000.png" ]] || continue
  rec=$(rec_for "$lang")
  mkdir -p "$OUT/ppm/$lang" "$OUT/crops/$lang"
  "$PY" -I - "$dir" "$OUT/ppm/$lang" <<'PY'
import os, sys
from PIL import Image
src, dst = sys.argv[1], sys.argv[2]
for n in sorted(os.listdir(src)):
    if n.endswith(".png"):
        Image.open(os.path.join(src, n)).convert("RGB").save(os.path.join(dst, n[:-4] + ".ppm"))
PY
  if [[ $rec == tess ]]; then
    OCR_LABEL_PREFIX="$lang/" "$OCR_BIN" pipeline "$OCR_DIR/PP-OCRv6_small_det_safetensors" \
      "tess:$OCR_DIR/tessdata_best/heb.traineddata" "$OUT/ours-$lang.tsv" --side "$SIDE" \
      --crops "$OUT/crops/$lang" "$OUT/ppm/$lang"/*.ppm
    # The reference: Tesseract on the same line images (our detector's boxes).
    "$PY" -I - "$OUT/crops/$lang" <<'PY'
import os, sys
from PIL import Image
d = sys.argv[1]
for n in os.listdir(d):
    if n.endswith(".pgm"):
        Image.open(os.path.join(d, n)).save(os.path.join(d, n[:-4] + ".png"))
PY
    PATH="$TESS_PREFIX/bin:$PATH" LD_LIBRARY_PATH="$TESS_PREFIX/lib" "$PY" -I "$HERE/ocr-oracle.py" \
      tess "$OCR_DIR/tessdata_best" heb "$OUT/crops/$lang" "$OUT/tess-$lang.tsv"
    # Same boxes as ours, Tesseract's text.
    "$PY" -I - "$OUT/ours-$lang.tsv" "$OUT/tess-$lang.tsv" "$lang" >> "$OUT/ref.tsv" <<'PY'
import sys
ours, tess, lang = sys.argv[1], sys.argv[2], sys.argv[3]
texts = {}
for l in open(tess, encoding="utf-8"):
    p = l.rstrip("\n").split("\t")
    texts[p[0][:-4]] = p[1] if len(p) > 1 else ""
counts = {}
for l in open(ours, encoding="utf-8"):
    p = l.rstrip("\n").split("\t")
    img = p[0].split("/")[-1][:-4]
    n = counts.get(img, 0)
    counts[img] = n + 1
    t = texts.get(f"{img}-{n:03}", "")
    print("\t".join(p[:9] + ["1", t]))
PY
  else
    OCR_LABEL_PREFIX="$lang/" "$OCR_BIN" pipeline "$OCR_DIR/PP-OCRv6_small_det_safetensors" \
      "$OCR_DIR/${rec}_safetensors" "$OUT/ours-$lang.tsv" --side "$SIDE" "$OUT/ppm/$lang"/*.ppm
    PADDLE_PDX_CACHE_HOME="$OUT/pdx-cache" "$PADDLE_PY" -I "$HERE/ocr-oracle.py" paddle \
      "$OCR_DIR/paddle/PP-OCRv6_small_det" "$OCR_DIR/paddle/$rec" "$dir" "$OUT/ref.tsv" \
      --side "$SIDE" --prefix "$lang/"
  fi
  cat "$OUT/ours-$lang.tsv" >> "$OUT/ours.tsv"
done
"$PY" -I "$HERE/ocr-oracle.py" score "$SYNTH_DIR/truth.tsv" "$OUT/ours.tsv" "$OUT/ref.tsv"
