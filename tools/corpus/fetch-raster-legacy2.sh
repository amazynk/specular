#!/usr/bin/env bash
# Generate the XWD / TIM / MIFF oracle corpus into tools/corpus/raster/legacy2/ (gitignored)
# and ImageMagick's own reading of every image beside it (the oracle):
#   files/<name>                 the test file
#   expect/<name>.<page>.rgba    ImageMagick's decode of that page: 16-bit RGBA, big-endian
#   expect/<name>.pages          the number of images ImageMagick finds
#
# Usage: tools/corpus/fetch-raster-legacy2.sh
#   Needs `magick` (ImageMagick 7 with MIFF, XWD and TIM) and python3.
#   Then: SPECULAR_RASTER_CORPUS_LEGACY2=tools/corpus/raster/legacy2 \
#     cargo test -p specular-legacy2 --test oracle -- --ignored --nocapture
#
# Everything is generated locally (nothing is downloaded): MIFF in every compression, depth,
# float format, class, colour space, alpha and byte order ImageMagick writes, plus multi-image
# files; XWD as ImageMagick writes it (TrueColor, PseudoColor, grey, 1-bit); TIM files in all
# four pixel modes (with several CLUTs) written by the Python below.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RASTER_CORPUS_LEGACY2:-$ROOT/tools/corpus/raster/legacy2}
F="$OUT/files"
E="$OUT/expect"
rm -rf "$F" "$E" "$OUT/twin"
mkdir -p "$F" "$E" "$OUT/twin"
command -v magick >/dev/null || { echo "fetch-raster-legacy2: needs magick" >&2; exit 1; }

# Sources: a photo-like picture, a smooth gradient with a soft alpha ramp, an odd-sized one.
S="$OUT/src"
mkdir -p "$S"
magick rose: "$S/rose.png"
magick -size 61x37 gradient:'#ff2000-#0030ff' \( -size 37x61 gradient:white-'#202020' -rotate 90 \) \
  -alpha off -compose CopyOpacity -composite "$S/alpha.png"
magick -size 129x3 gradient:black-white "$S/wide.png"

n=0
miff() { # name, input, options...
  local name=$1 in=$2
  shift 2
  magick "$in" "$@" "$F/$name.miff"
  n=$((n + 1))
}
for c in None RLE Zip BZip; do
  for d in 8 16 32; do
    miff "rose-$c-$d" "$S/rose.png" -compress $c -depth $d
    miff "alpha-$c-$d" "$S/alpha.png" -compress $c -depth $d
    miff "pal-$c-$d" "$S/rose.png" -colors 200 -type palette -compress $c -depth $d
    miff "palalpha-$c-$d" "$S/alpha.png" -colors 60 -type palettealpha -compress $c -depth $d
    miff "grey-$c-$d" "$S/rose.png" -type grayscale -compress $c -depth $d
    miff "greyalpha-$c-$d" "$S/alpha.png" -type grayscalealpha -compress $c -depth $d
    miff "cmyk-$c-$d" "$S/rose.png" -colorspace CMYK -compress $c -depth $d
    miff "wide-$c-$d" "$S/wide.png" -compress $c -depth $d
    miff "lsb-$c-$d" "$S/alpha.png" -endian LSB -compress $c -depth $d
  done
  for d in 16 32 64; do
    miff "float-$c-$d" "$S/alpha.png" -define quantum:format=floating-point -compress $c -depth $d
  done
  miff "multi-$c" "$S/rose.png" "$S/alpha.png" "$S/wide.png" -compress $c
done
miff "linear-None-16" "$S/rose.png" -colorspace RGB -depth 16
# Every other colour space ImageMagick writes (shown converted to sRGB), at 8 and 16 bits and
# as floats, with alpha; meta channels; 64-bit integers.
for cs in Adobe98 CAT02LMS CIELab CMY DisplayP3 HCL HCLp HSB HSI HSL HSV HWB Jzazbz Lab LCH \
          LCHab LCHuv LMS Log Luv OHTA Oklab Oklch ProPhoto Rec601YCbCr Rec709YCbCr scRGB \
          Transparent xyY XYZ YCbCr YDbDr YCC YIQ YPbPr YUV; do
  miff "cs-$cs-16" "$S/rose.png" -colorspace $cs -depth 16
  miff "cs-$cs-8" "$S/rose.png" -colorspace $cs -depth 8
  miff "cs-$cs-a32" "$S/alpha.png" -colorspace $cs -define quantum:format=floating-point -depth 32
done
miff "meta-None-8" "$S/rose.png" -channel-fx 'red=>meta' -depth 8
miff "meta-Zip-16" "$S/alpha.png" -channel-fx 'red=>meta' -compress Zip -depth 16
miff "meta-grey-16" "$S/rose.png" -type grayscale -channel-fx 'gray=>meta' -depth 16

# XWD as ImageMagick writes it.
magick "$S/rose.png" "$F/rose24.xwd"
magick "$S/rose.png" -colors 100 -type palette "$F/rose-pal.xwd"
magick "$S/rose.png" -type grayscale "$F/rose-grey.xwd"
magick "$S/rose.png" -monochrome "$F/rose-mono.xwd"
magick "$S/wide.png" "$F/wide.xwd"
magick "$S/alpha.png" "$F/alpha.xwd"

# TIM files in every pixel mode.
python3 -I - "$F" <<'PY'
import struct, sys, os
out = sys.argv[1]
def block(x, y, w, h, data):
    return struct.pack('<IHHHH', 12 + len(data), x, y, w, h) + data
def c15(r, g, b, stp=0):
    return (r >> 3) | ((g >> 3) << 5) | ((b >> 3) << 10) | (stp << 15)
def tim(name, mode, clut, w_units, h, data):
    flags = mode | (8 if clut is not None else 0)
    f = struct.pack('<II', 0x10, flags)
    if clut is not None:
        f += clut
    f += block(320, 0, w_units, h, data)
    open(os.path.join(out, name), 'wb').write(f)
for (bits, mode) in ((4, 0), (8, 1)):
    n = 1 << bits
    for nclut in (1, 3):
        cl = b''.join(struct.pack('<H', c15((i * 37 + k * 50) & 255, (i * 91) & 255, (255 - i * 13 + k) & 255, i & 1))
                      for k in range(nclut) for i in range(n))
        clut = block(0, 480, n, nclut, cl)
        # ImageMagick reads only the first CLUT and then loses its place: its oracle reads a
        # twin with that CLUT alone (twin/), which must decode the same.
        clut1 = block(0, 480, n, 1, cl[:2 * n])
        for (w, h) in ((32, 17), (24, 9)):
            px = [(x * 3 + y * 5) % n for y in range(h) for x in range(w)]
            if bits == 4:
                data = bytes(px[i] | (px[i + 1] << 4) for i in range(0, len(px), 2))
                units = w // 4
            else:
                data = bytes(px)
                units = w // 2
            tim(f'tim{bits}-c{nclut}-{w}x{h}.tim', mode, clut, units, h, data)
            if nclut > 1:
                tim(f'../twin/tim{bits}-c{nclut}-{w}x{h}.tim', mode, clut1, units, h, data)
for (w, h) in ((40, 21), (7, 3)):
    data = b''.join(struct.pack('<H', c15((x * 6) & 255, (y * 12) & 255, (x * y) & 255, (x + y) & 1)) for y in range(h) for x in range(w))
    tim(f'tim16-{w}x{h}.tim', 2, None, w, h, data)
for (w, h) in ((40, 21), (2, 5)):
    row = lambda y: bytes(v for x in range(w) for v in ((x * 6) & 255, (y * 12) & 255, (x * y * 3) & 255))
    data = b''.join(row(y) for y in range(h))
    tim(f'tim24-{w}x{h}.tim', 3, None, w * 3 // 2, h, data)
PY

# The oracle: ImageMagick's decode of every page as 16-bit RGBA.
for f in "$F"/*; do
  b=$(basename "$f")
  [[ -e "$OUT/twin/$b" ]] && f="$OUT/twin/$b"
  pages=$(magick identify -format '%p\n' "$f" 2>/dev/null | wc -l || true)
  echo "$pages" > "$E/$b.pages"
  for ((p = 0; p < pages; p++)); do
    # Linear RGB is compared as shown (sRGB-encoded); ImageMagick's raw RGBA would stay linear.
    cs=(); [[ $b == linear-* || $b == cs-* ]] && cs=(-colorspace sRGB)
    magick "$f[$p]" "${cs[@]}" -depth 16 -endian MSB "RGBA:$E/$b.$p.rgba" 2>/dev/null || rm -f "$E/$b.$p.rgba"
  done
done
# ImageMagick 7.1.2 reads its own PseudoClass RLE files as black (its reader drops the colour
# map lookup): their oracle is the same image written uncompressed.
for f in "$E"/pal*-RLE-*.miff.0.rgba; do
  cp "${f/-RLE-/-None-}" "$f"
done
rm -rf "$S" "$OUT/twin"
echo "fetch-raster-legacy2: $(ls "$F" | wc -l) files in $F"
