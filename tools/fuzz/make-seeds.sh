#!/usr/bin/env bash
# Regenerate the small synthetic seed files committed under `<crate>/fuzz/seeds/`.
#
#   tools/fuzz/make-seeds.sh [crate name...]     # e.g. bitmap png; no argument = all
#
# Seeds are tiny (a few hundred bytes) images made from a synthetic pattern, so they are
# redistributable. Needs ImageMagick 7 (`magick`), python3, and for some crates `cwebp`,
# `heif-enc` (with x265), `exiftool`. CI does not run this: it uses the committed seeds.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
work=$root/target/fuzz-seeds
mkdir -p "$work"
src=$work/src.png
magick -size 7x5 plasma:fractal -seed 1 -depth 8 "$src" 2>/dev/null ||
  magick -seed 1 -size 7x5 plasma:fractal -depth 8 "$src"
src2=$work/src2.png
magick -seed 2 -size 7x5 plasma:fractal -depth 8 "$src2"
alpha=$work/alpha.png
magick "$src" \( -size 7x5 gradient:white-black \) -alpha off -compose copyopacity -composite "$alpha"
# Compact ICC Profiles' sRGB-v2-micro (CC0), packaged by Debian/Ubuntu as icc-profiles-free.
icc=/usr/share/color/icc/sRGB-v2-micro.icc
pyw() { python3 -I "$root/tools/fuzz/seeds.py" "$@"; }

seeds_bitmap() {
  local d=$root/crates/formats/specular-bitmap/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  for f in bmp:a.bmp bmp2:a2.bmp bmp3:a3.bmp tga:a.tga qoi:a.qoi ppm:a.ppm pgm:a.pgm pbm:a.pbm \
    pam:a.pam gif:a.gif ico:a.ico cur:a.cur; do magick "$src" "$f"; done
  magick "$src" -colors 4 bmp3:pal4.bmp
  magick "$src" -colors 16 -compress RLE bmp3:rle8.bmp
  magick "$src" -compress RLE tga:rle.tga
  magick "$alpha" bmp:alpha.bmp
  magick "$alpha" qoi:alpha.qoi
  magick "$alpha" pam:alpha.pam
  magick "$alpha" tga:alpha.tga
  magick "$src" -compress none ppm:ascii.ppm
  magick "$src" -depth 16 ppm:d16.ppm
  magick -delay 5 -dispose previous "$src" -dispose background "$src2" -interlace GIF -loop 0 anim.gif
  pyw ani a.ico anim.ani
}

seeds_legacy() {
  local d=$root/crates/formats/specular-legacy/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  for f in pcx:a.pcx sgi:a.sgi sun:a.ras xbm:a.xbm xpm:a.xpm wbmp:a.wbmp farbfeld:a.ff; do
    magick "$src" "$f"
  done
  magick "$src" -colors 16 pcx:pal.pcx
  magick "$src" -compress RLE sgi:rle.sgi
  magick "$src" -compress RLE sun:rle.ras
  magick "$alpha" sgi:alpha.sgi
  magick "$alpha" farbfeld:alpha.ff
  magick "$src" -colors 8 sun:pal.ras
  magick "$alpha" xpm:alpha.xpm
  pyw fli fli a.fli
  pyw fli flc a.flc
}

seeds_radiance() {
  local d=$root/crates/formats/specular-radiance/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  magick "$src" hdr:flat.hdr
  magick -seed 3 -size 16x3 plasma:fractal hdr:rle.hdr
  magick "$src" pfm:rgb.pfm
  pyw radiance "$d"
}

seeds_texture() {
  local d=$root/crates/formats/specular-texture/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  magick "$alpha" -define dds:compression=dxt1 -define dds:mipmaps=1 dds:dxt1.dds
  magick "$alpha" -define dds:compression=dxt5 dds:dxt5.dds
  magick "$alpha" -define dds:compression=none dds:rgba.dds
  pyw texture "$d"
}

seeds_png() {
  local d=$root/crates/formats/specular-png/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  magick "$src" png24:rgb8.png
  magick "$alpha" png32:rgba8.png
  magick "$alpha" -depth 16 png64:rgba16.png
  magick "$src" -colorspace gray -depth 1 png:gray1.png
  magick "$src" -colorspace gray -depth 4 png:gray4.png
  magick "$src" -colorspace gray -depth 16 png:gray16.png
  magick "$alpha" -colorspace gray png:graya8.png
  magick "$alpha" -colors 6 png8:pal_trns.png
  magick "$src" -colors 3 -define png:bit-depth=2 png8:pal2.png
  magick -seed 4 -size 19x13 plasma:fractal -interlace PNG png24:adam7.png
  magick "$alpha" -interlace PNG -depth 16 png64:adam7_16.png
  magick "$src" -profile "$icc" png24:iccp.png
  magick -delay 3 -dispose previous "$alpha" -dispose background "$src2" -loop 2 apng:anim.png
  chmod 644 anim.png # the APNG writer creates it 0600
  pyw png "$d"
}

seeds_tiff() {
  local d=$root/crates/formats/specular-tiff/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  # ImageMagick picks a palette for few-colour images unless told otherwise; drop the dates.
  local t=(+set date:create +set date:modify -define tiff:exclude-tags=306)
  local rgb=("$src" -type TrueColor "${t[@]}")
  magick "${rgb[@]}" -compress none tif:rgb8.tif
  magick "$alpha" -type TrueColorAlpha "${t[@]}" -compress lzw -define tiff:predictor=2 tif:rgba8_lzw_pred.tif
  magick "${rgb[@]}" -depth 16 -compress zip -define tiff:predictor=2 -endian MSB tif:rgb16_zip_be.tif
  magick "${rgb[@]}" -compress rle tif:packbits.tif
  magick "${rgb[@]}" -compress jpeg -quality 70 -define tiff:tile-geometry=16x16 tif:jpeg_tiled.tif
  magick "${rgb[@]}" -depth 32 -define quantum:format=floating-point -compress zip \
    -define tiff:predictor=3 -define tiff:tile-geometry=16x16 tif:float_tiled.tif
  magick "$src" "${t[@]}" -colorspace gray -type bilevel -depth 1 -compress none tif:bilevel.tif
  magick "$src" "${t[@]}" -colorspace gray -type grayscale -depth 16 -compress lzw tif:gray16.tif
  magick "$src" "${t[@]}" -colors 5 -type palette tif:palette.tif
  magick "$src" "${t[@]}" -colorspace cmyk -compress lzw tif:cmyk.tif
  magick "${rgb[@]}" -interlace plane -compress none tif:planar.tif
  magick "${rgb[@]}" -compress none TIFF64:big.tif
  magick -seed 5 -size 40x24 plasma:fractal "${t[@]}" -compress zip ptif:pyramid.tif
  magick "$src" "$src2" -type TrueColor "${t[@]}" -compress lzw tif:pages.tif
  pyw tiff "$d"
}

seeds_psd() {
  local d=$root/crates/formats/specular-psd/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  local t=(+set date:create +set date:modify)
  magick "$src" -type TrueColor "${t[@]}" -compress rle psd:rgb8_rle.psd
  magick "$src" -type TrueColor "${t[@]}" -compress none psd:rgb8_raw.psd
  magick "$alpha" -type TrueColorAlpha "${t[@]}" -compress zip psd:rgba8_zip.psd
  magick "$alpha" "$src2" -type TrueColorAlpha "${t[@]}" -compress rle psd:layers.psd
  magick "$src" -type TrueColor "${t[@]}" -depth 16 -compress rle psd:rgb16.psd
  magick "$src" "${t[@]}" -colorspace gray -type grayscale -compress rle psd:gray8.psd
  magick "$src" "${t[@]}" -colors 5 -type palette psd:indexed.psd
  magick "$src" "${t[@]}" -colorspace cmyk -compress rle psd:cmyk.psd
  magick "$src" -type TrueColor "${t[@]}" -compress rle psb:big.psb
  magick "$src" -resize 4x3! -strip -quality 50 "$work/thumb.jpg"
  pyw psd "$d" "$work/thumb.jpg"
}

seeds_exr() {
  local d=$root/crates/formats/specular-exr/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  local c
  for c in none rle zips zip piz pxr24 b44 b44a; do magick "$alpha" -compress $c exr:half_$c.exr; done
  magick "$src" -define exr:color-type=Y -compress zip exr:luma.exr
  magick -seed 3 -size 8x6 plasma:fractal -define exr:color-type=YCA -compress none exr:ycc.exr
  pyw exr "$d"
}

seeds_jp2() {
  local d=$root/crates/formats/specular-jp2/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  # Larger cases come from tests/data via fuzz/seed-fixtures.
  magick "$src" jp2:rgb.jp2
  magick "$alpha" jp2:rgba.jp2
  magick "$src" -colorspace gray j2k:gray.j2k
  magick "$src" -depth 16 j2k:rgb16.j2k
  # HTJ2K (Part 15) cannot be written by ImageMagick; copy the two tiny openjpeg-data files
  # (BSD-2-Clause) when tools/corpus/fetch-conformance.sh has fetched them.
  local ht=$root/tools/corpus/conformance/openjpeg-data/input/nonregression/htj2k
  [ -f "$ht/byte.jph" ] && cp "$ht/byte.jph" "$ht/byte_causal.jhc" .
  return 0
}

seeds_webp() {
  local d=$root/crates/formats/specular-webp/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  # Lossy and animated cases come from tests/data via fuzz/seed-fixtures.
  cwebp -quiet -lossless "$alpha" -o lossless_alpha.webp
  cwebp -quiet -q 60 -alpha_q 50 "$alpha" -o lossy_alpha.webp
  cwebp -quiet -q 60 -alpha_method 0 -alpha_filter none "$alpha" -o lossy_alpha_raw.webp
  cwebp -quiet -q 70 -metadata all "$src" -o lossy.webp
  cwebp -quiet -lossless -exact -z 9 "$src2" -o lossless_z9.webp
  magick "$src" -profile "$icc" -quality 60 webp:iccp.webp
}

seeds_archive() {
  local d=$root/crates/specular-archive/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  magick "$src" -resize 2x2! -strip png24:"$work/p1.png"
  magick "$src2" -resize 2x2! -strip png24:"$work/p2.png"
  pyw archive "$d" "$work/p1.png" "$work/p2.png"
  # 7z: copy, LZMA, LZMA2 (solid), PPMd, Deflate, a BCJ filter chain, and an uncompressed header.
  local s=$work/7z
  rm -rf "$s" && mkdir -p "$s/sub" && cp "$work/p1.png" "$s/01.png" && cp "$work/p2.png" "$s/sub/02.png"
  printf '<ComicInfo><Manga>Yes</Manga></ComicInfo>' >"$s/ComicInfo.xml"
  touch -d 2020-01-02T03:04:06Z "$s"/* "$s"/sub/*
  local m
  rm -f ./*.7z ./*.cb7
  for m in copy:-m0=Copy lzma:-m0=LZMA lzma2_solid:-m0=LZMA2:d=64k ppmd:-m0=PPMd:mem=1m deflate:-m0=Deflate \
    bcj:-mf=BCJ; do
    (cd "$s" && 7z a -bd -bso0 -mtm=off -mtc=off -mta=off "${m#*:}" "$d/${m%%:*}.7z" 01.png sub ComicInfo.xml)
  done
  (cd "$s" && 7z a -bd -bso0 -mtm=off -mx=9 -ms=off -mhc=off "$d/nonsolid_plainhdr.cb7" 01.png sub ComicInfo.xml)
}

seeds_icns() {
  local d=$root/crates/formats/specular-icns/fuzz/seeds
  mkdir -p "$d"
  magick "$src" -resize 4x4! -strip png32:"$work/icns.png"
  pyw icns "$d" "$work/icns.png"
}

seeds_ora() {
  local d=$root/crates/formats/specular-ora/fuzz/seeds
  mkdir -p "$d"
  magick "$src" -resize 2x2! -strip png32:"$work/o1.png"
  magick "$alpha" -resize 2x2! -strip png32:"$work/o2.png"
  pyw ora "$d" "$work/o1.png" "$work/o2.png"
}

seeds_iff() {
  # netpbm's ppmtoilbm: indexed (ByteRun1 and raw), HAM6/HAM8, 24-bit deep, a mask plane and
  # a transparent colour.
  local d=$root/crates/formats/specular-iff/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  magick "$src" ppm:"$work/iff.ppm"
  magick "$src" -colors 8 ppm:"$work/iff8.ppm"
  ppmtoilbm "$work/iff8.ppm" >indexed.iff 2>/dev/null
  ppmtoilbm -cmethod none "$work/iff8.ppm" >raw.iff 2>/dev/null
  ppmtoilbm -ham6 "$work/iff.ppm" >ham6.iff 2>/dev/null
  ppmtoilbm -ham8 "$work/iff.ppm" >ham8.iff 2>/dev/null
  ppmtoilbm -24force "$work/iff.ppm" >deep24.iff 2>/dev/null
  ppmtoilbm -mmethod maskplane "$work/iff8.ppm" >mask.iff 2>/dev/null
  ppmtoilbm -mmethod transparentcolor -transparent black "$work/iff8.ppm" >transparent.iff 2>/dev/null
}

seeds_pict() {
  # ImageMagick (32-bit direct pack type 4, 8-bit PackBits, QuickTime JPEG) and netpbm's
  # ppmtopict; one of each also without the 512-byte file header.
  local d=$root/crates/formats/specular-pict/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  magick "$src" pict:rgb.pict
  magick "$src" -colors 16 pict:pal.pict
  magick "$src" -compress JPEG -quality 50 pict:jpeg.pict
  # ppmtopict wants 8 pixels or more across.
  magick "$src" -resize 9x5! -colors 200 ppm:- | ppmtopict >ppmtopict.pict 2>/dev/null
  tail -c +513 rgb.pict >rgb_noheader.pict
  tail -c +513 ppmtopict.pict >ppmtopict_noheader.pict
}

seeds_metadata() {
  # The jpeg target uses tests/fixtures via fuzz/seed-fixtures; the payloads target gets the
  # same EXIF/XMP/ICC payloads cut out of them.
  local c=$root/crates/specular-metadata
  pyw metadata "$c/fuzz/seeds/payloads" "$icc" "$c"/tests/fixtures/*.jpg
}

seeds_decode() {
  # Where XMP and ICC profiles sit in PNG, WebP, HEIC/AVIF, TIFF and JPEG XL headers; the
  # metadata fixtures of specular-thumbs come in via fuzz/seed-fixtures.
  pyw embedded "$root/crates/specular-decode/fuzz/seeds"
}

seeds_color() {
  # Synthetic ICC profiles: v2/v4 matrix/TRC RGB, gray, lut8/lut16, lutAtoB/lutBtoA RGB and CMYK.
  pyw icc "$root/crates/specular-color/fuzz/seeds/profile"
}

seeds_heic() {
  # x265 RExt: 4:2:0, 4:2:2 and 4:4:4 at 8/10/12 bit, monochrome, and an alpha auxiliary image.
  local d=$root/crates/formats/specular-heic/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  local p8=$work/heic8.png p16=$work/heic16.png
  magick -seed 1 -size 16x16 plasma:fractal -depth 8 "$p8"
  magick -seed 1 -size 16x16 plasma:fractal -depth 16 "$p16"
  magick "$p8" -colorspace gray "$work/heicg.png"
  magick "$p8" \( -size 16x16 gradient:white-black \) -alpha off -compose copyopacity -composite "$work/heica.png"
  local c enc=(heif-enc -q 40 -e x265)
  for c in 420 422 444; do
    "${enc[@]}" -p chroma=$c "$p8" -o c$c.heic >/dev/null
    "${enc[@]}" -p chroma=$c -b 10 "$p16" -o c${c}_10.heic >/dev/null
  done
  "${enc[@]}" -p chroma=420 -b 12 "$p16" -o c420_12.heic >/dev/null
  "${enc[@]}" -p chroma=444 -b 12 "$p16" -o c444_12.heic >/dev/null
  "${enc[@]}" "$work/heicg.png" -o mono.heic >/dev/null
  "${enc[@]}" "$work/heica.png" -o alpha.heic >/dev/null
}

seeds_jpeg() {
  # Baseline 4:2:0/4:2:2/4:4:4, progressive, greyscale, CMYK, restart markers, arithmetic coding.
  local d=$root/crates/formats/specular-jpeg/fuzz/seeds
  mkdir -p "$d" && cd "$d"
  local j=(-strip -quality 60)
  magick "$src" "${j[@]}" -sampling-factor 2x2 jpg:s420.jpg
  magick "$src" "${j[@]}" -sampling-factor 2x1 jpg:s422.jpg
  magick "$src" "${j[@]}" -sampling-factor 1x1 jpg:s444.jpg
  magick -seed 4 -size 19x13 plasma:fractal "${j[@]}" -interlace JPEG jpg:progressive.jpg
  magick "$src" "${j[@]}" -colorspace gray jpg:gray.jpg
  magick "$src" "${j[@]}" -colorspace cmyk jpg:cmyk.jpg
  magick -seed 4 -size 19x13 plasma:fractal "${j[@]}" -define jpeg:restart-interval=1 jpg:restart.jpg
  magick "$src" -profile "$icc" -quality 60 jpg:iccp.jpg
  if command -v cjpeg >/dev/null; then
    magick "$src" ppm:- | cjpeg -arithmetic -quality 60 >arith.jpg
    magick -seed 4 -size 19x13 plasma:fractal ppm:- |
      cjpeg -arithmetic -progressive -restart 1 -quality 60 >arith-prog-rst.jpg
  fi
}

seeds_brotli() {
  # Each quality, a small and a large window, stored meta-blocks, dictionary words, empty.
  # The `decompress` target reads the first byte as its output cap selector, so its seeds get a
  # leading 0x05 (1 MiB); `differential` takes the bare stream.
  local d=$root/crates/formats/specular-brotli/fuzz/seeds
  mkdir -p "$d/decompress" "$d/differential"
  python3 -I - "$d" <<'EOF'
import brotli, os, random, sys
d = sys.argv[1]
r = random.Random(1)
text = b"The time of the people, however, between the information and the following " * 3
html = b'<div class="a"><a href="http://www.example.com/">International</a></div>\n'
noise = bytes(r.getrandbits(8) for _ in range(120))
cases = [("empty", b"", 11, 22), ("noise", noise, 5, 16), ("html", html, 11, 22)]
cases += [(f"q{q}", text, q, 10 + q) for q in range(12)]
for name, data, q, w in cases:
    c = brotli.compress(data, quality=q, lgwin=w)
    for t, pre in (("decompress", b"\x05"), ("differential", b"")):
        with open(os.path.join(d, t, name + ".br"), "wb") as f:
            f.write(pre + c)
EOF
}

# --- end of seed functions ---
all=$(declare -F | sed -n 's/^declare -f seeds_//p')
# shellcheck disable=SC2086 # one function name per word
[ $# -gt 0 ] || set -- $all
for c in "$@"; do
  echo "seeds: $c"
  "seeds_$c"
done
