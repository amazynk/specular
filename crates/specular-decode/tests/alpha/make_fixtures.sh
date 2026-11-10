#!/usr/bin/env bash
# Regenerates the cross-format alpha grid fixtures (tests/alpha_grid.rs) in this directory.
# grid.py writes the reference PNGs and every format whose bytes need exact control (TIFF
# associated/unassociated alpha, premultiplied EXR, BMP bitfields, ICO/CUR masks, TGA alpha
# types, BCn and uncompressed DDS/KTX2, APNG); the external encoders below make the rest from
# those PNGs. Run it only to change a fixture on purpose; CI uses the committed files.
#
# Made with ImageMagick 7.1.2, cwebp (libwebp 1.5.0), heif-enc (libheif 1.21.2 with aom 3.13.1
# and x265 4.1) and ffmpeg 7 with libjxl.
set -euo pipefail
cd "$(dirname "$0")"
python3 -I grid.py .

# PNG sources: 8-bit, 16-bit and the 1-bit (thresholded) variant.
p8=rgba8.png p16=rgba16.png p1=rgba8-1bit.png

# GIF: one transparent index, so the 1-bit variant.
magick "$p1" gif:index.gif

# WebP: lossless keeping the colour under transparent pixels, and lossy with lossless alpha.
cwebp -quiet -lossless -exact -z 6 "$p8" -o lossless.webp
cwebp -quiet -q 90 -alpha_q 100 "$p8" -o lossy.webp

# AVIF (alpha as an auxiliary image): 8, 10 and 12 bits, and 8-bit lossless.
heif-enc --avif -q 90 -o avif8.avif "$p8" >/dev/null
heif-enc --avif -q 90 -b 10 -o avif10.avif "$p16" >/dev/null
heif-enc --avif -q 90 -b 12 -o avif12.avif "$p16" >/dev/null
heif-enc --avif -L -o avif8-lossless.avif "$p8" >/dev/null

# HEIC with alpha, 8 and 10 bits.
heif-enc -q 90 -o heic8.heic "$p8" >/dev/null
heif-enc -q 90 -b 10 -o heic10.heic "$p16" >/dev/null

# JPEG XL: lossless 8 and 16 bits, lossy 8-bit.
ff() { ffmpeg -hide_banner -loglevel error -y "$@"; }
ff -i "$p8" -c:v libjxl -distance 0 -effort 7 jxl8.jxl
ff -i "$p16" -c:v libjxl -distance 0 -effort 7 -pix_fmt rgba64le jxl16.jxl
ff -i "$p8" -c:v libjxl -distance 1 -effort 7 jxl8-lossy.jxl

# Photoshop: merged image plus layer, 8 and 16 bits.
magick "$p8" psd:rgba8.psd
magick "$p16" -depth 16 psd:rgba16.psd

# Other ImageMagick writers with alpha.
magick "$p8" qoi:rgba8.qoi
magick "$p8" pam:rgba8.pam
magick "$p16" -depth 16 pam:rgba16.pam
magick "$p8" sgi:rgba8.sgi
magick "$p16" -depth 16 sgi:rgba16.sgi
magick "$p16" farbfeld:rgba16.ff
magick "$p1" xpm:index.xpm
magick "$p8" jp2:rgba8.jp2
# 16-bit with no wavelet levels, so reduced decodes take the box filter (downscale_wide).
magick "$p16" -depth 16 -define jp2:number-resolutions=1 -quality 0 jp2:rgba16-r1.jp2
magick "$p8" -define tiff:alpha=unassociated -compress lzw tiff:magick-unassoc.tif
magick "$p8" -define dds:compression=dxt5 -define dds:mipmaps=0 dds:magick-dxt5.dds
magick "$p8" bmp:magick.bmp
magick "$p8" tga:magick.tga
magick "$p8" ico:magick.ico

ls -l
