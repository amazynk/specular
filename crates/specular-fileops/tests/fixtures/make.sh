#!/usr/bin/env bash
# Regenerates the rotation fixtures: small images of each format whose orientation the
# permanent rotation writes. ImageMagick encodes them; exiftool adds Exif to some.
# Needs: magick (ImageMagick 7 with libheif and libwebp), exiftool. Writes next to this script.
set -euo pipefail
cd "$(dirname "$0")"
src=$(mktemp -p . rot-src.XXXXXX.ppm)
trap 'rm -f "$src"' EXIT
# An asymmetric picture, so every orientation looks different.
mk() {
  magick -size "$1" gradient:red-blue -fill yellow -draw 'rectangle 0,0 5,3' \
    -fill black -draw 'rectangle 0,8 2,15' "ppm:$src"
}
mk 32x16
magick "ppm:$src" -strip -sampling-factor 2x2 -quality 85 jpg:aligned.jpg
exiftool -q -overwrite_original -Orientation=1 -n -Make=Specular -Model='Test rig' aligned.jpg
magick "ppm:$src" -strip -sampling-factor 2x2 -quality 85 jpg:noexif.jpg
magick "ppm:$src" -strip png:plain.png
magick "ppm:$src" -strip -quality 80 webp:lossy.webp
magick "ppm:$src" -strip -define webp:lossless=true webp:lossless.webp
magick "ppm:$src" -strip -compress zip -endian LSB tiff:little.tif
magick "ppm:$src" -strip -compress none -endian MSB tiff:big.tif
magick "ppm:$src" -strip -quality 60 heic:plain.heic
magick "ppm:$src" -strip -quality 60 avif:plain.avif
mk 27x19
magick "ppm:$src" -strip -sampling-factor 2x2 -quality 85 jpg:odd.jpg
exiftool -q -overwrite_original -Orientation=6 -n odd.jpg
magick "ppm:$src" -strip png:odd.png
exiftool -q -overwrite_original -Orientation=3 -n odd.png
ls -l ./*.jpg ./*.png ./*.webp ./*.tif ./*.heic ./*.avif
