#!/bin/sh
# Regenerates the L*a*b* TIFF fixtures with ImageMagick 6 (`convert`), from its built-in
# "rose:" image (70×46). tests/lab.rs compares their decode with the RGB reference.
#
#   rose-rgb.tif         the reference, 8-bit RGB
#   rose-lab8.tif        CIELab 8-bit, LZW
#   rose-lab16.tif       CIELab 16-bit, Deflate (ImageMagick scales 16-bit a*/b* by 257, not
#                        the 256 of the TIFF technical note; under 0.4 % apart)
#   rose-lab8-icc.tif    rose-lab8.tif with an RGB profile attached (ignored by the decoder)
#   rose-lab8-tiled.tif  CIELab 8-bit, 32×32 tiles, PackBits
#   rose-lab12.tif       CIELab 12-bit, Deflate (ImageMagick 7)
#   rose-lab4.tif        CIELab 4-bit, uncompressed (ImageMagick 7)
#   rose-lab4-rgb.tif    ImageMagick 7's own sRGB decode of rose-lab4.tif (its reference)
#   rose-labf.tif        CIELab 32-bit float, Deflate (ImageMagick 7: the 16-bit coding over 0-1)
set -eu
cd "$(dirname "$0")"
convert rose: -depth 8 -compress none rose-rgb.tif
convert rose: -colorspace Lab -depth 8 -compress lzw rose-lab8.tif
convert rose: -colorspace Lab -depth 16 -compress zip rose-lab16.tif
convert rose: -colorspace Lab -depth 8 -compress lzw \
    -set profile /usr/share/color/icc/AdobeCompat-v2.icc rose-lab8-icc.tif
convert rose: -colorspace Lab -depth 8 -define tiff:tile-geometry=32x32 \
    -compress rle rose-lab8-tiled.tif
magick rose: -colorspace Lab -depth 12 -compress zip rose-lab12.tif
magick rose: -colorspace Lab -depth 4 -compress none rose-lab4.tif
magick rose-lab4.tif -colorspace sRGB -depth 8 -compress none rose-lab4-rgb.tif
magick rose: -colorspace Lab -depth 32 -define quantum:format=floating-point -compress zip \
    rose-labf.tif
