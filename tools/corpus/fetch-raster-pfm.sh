#!/usr/bin/env bash
# Make the PFM oracle corpus in tools/corpus/raster/pfm/ (gitignored): PFM files written by
# ImageMagick (grey and colour), Pillow (grey) and by hand (big-endian, odd sizes, CR LF after
# the header), each with ImageMagick's reading of it as raw little-endian float RGB beside it
# (<name>.rgbf, top row first). This ImageMagick build is not HDRI (it clamps to 0..1), so the
# hand-made files (values from -1 to 9) carry the generator's own values instead, and
# ImageMagick's clamped reading beside them as <name>.im.rgbf.
#
# Usage: tools/corpus/fetch-raster-pfm.sh
#   PYTHON (default python3) must have Pillow and numpy; a venv under target/ is fine.
# Then: SPECULAR_RASTER_CORPUS_PFM=tools/corpus/raster/pfm \
#   cargo test -p specular-decode --test hdr_formats pfm_corpus_matches_imagemagick -- --ignored --nocapture
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RASTER_CORPUS_PFM:-$ROOT/tools/corpus/raster/pfm}
PYTHON=${PYTHON:-python3}
command -v magick >/dev/null || { echo "fetch-raster-pfm: needs ImageMagick 7 (magick)" >&2; exit 1; }
mkdir -p "$OUT"

# ImageMagick: colour and grey from synthetic gradients, several sizes.
for size in 1x1 7x3 64x48 257x129; do
  magick -size "$size" gradient:red-blue -depth 16 "$OUT/im-rgb-$size.pfm"
  magick -size "$size" gradient:white-black -colorspace Gray "$OUT/im-grey-$size.pfm"
  magick -size "$size" plasma:fractal -seed 7 -evaluate multiply 3.5 "$OUT/im-hdr-$size.pfm"
done

# Pillow (mode F, grey) and hand-made files: big- and little-endian, colour and grey, a
# CR LF header and a scale other than one.
"$PYTHON" -I - "$OUT" <<'EOF'
import sys, struct, numpy as np
from PIL import Image
out = sys.argv[1]
rng = np.random.default_rng(3)
for (w, h) in [(5, 4), (33, 17)]:
    a = (rng.random((h, w)) * 4).astype(np.float32)
    Image.fromarray(a, mode="F").save(f"{out}/pil-grey-{w}x{h}.pfm")
    np.repeat(a[:, :, None], 3, axis=2).astype("<f4").tofile(f"{out}/pil-grey-{w}x{h}.rgbf")
    for grey in (True, False):
        ch = 1 if grey else 3
        v = (rng.random((h, w, ch)) * 10 - 1).astype(np.float32)
        for big in (True, False):
            for crlf in (False, True):
                magic = b"Pf" if grey else b"PF"
                nl = b"\r\n" if crlf else b"\n"
                # ImageMagick divides by the scale's magnitude and misreads a CR LF header,
                # so only the plain unit-scale files get its reading beside them.
                if crlf:
                    scale = b"2.5" if big else b"-0.25"
                else:
                    scale = b"1.0" if big else b"-1.0"
                head = magic + b"\n" + f"{w} {h}".encode() + b"\n" + scale + nl
                rows = v[::-1].astype(">f4" if big else "<f4").tobytes()
                name = f"hand-{'grey' if grey else 'rgb'}-{'be' if big else 'le'}{'-crlf' if crlf else ''}-{w}x{h}.pfm"
                with open(f"{out}/{name}", "wb") as f:
                    f.write(head + rows)
                rgb = np.repeat(v, 3, axis=2) if grey else v
                rgb.astype("<f4").tofile(f"{out}/{name[:-4]}.rgbf")
EOF

# The oracle: ImageMagick's reading as float RGB, normalised (1.0 = quantum range).
for f in "$OUT"/*.pfm; do
  dest=${f%.pfm}.rgbf
  if [[ $f == *-crlf-* ]]; then
    magick identify -format '%w %h\n' "$f" > "${f%.pfm}.size"
    continue
  fi
  [[ -e $dest ]] && dest=${f%.pfm}.im.rgbf
  magick "$f" -colorspace sRGB -set colorspace RGB -depth 32 -define quantum:format=floating-point \
    -endian LSB "RGB:$dest"
  magick identify -format '%w %h\n' "$f" > "${f%.pfm}.size"
done
echo "fetch-raster-pfm: $(ls "$OUT"/*.pfm | wc -l) files in $OUT"
