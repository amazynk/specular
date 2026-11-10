#!/usr/bin/env bash
# Build the DPX / Cineon test corpus under tools/corpus/raster/film/ (gitignored) and the
# oracles' readings of every file beside it.
#
# Usage: tools/corpus/fetch-raster-film.sh
#   OUT defaults to $SPECULAR_RASTER_CORPUS_FILM, else tools/corpus/raster/film.
#   MAGICK (default magick) is ImageMagick 7 with DPX and CIN. FFMPEG (default ffmpeg when found;
#   empty to skip) reads every file as a third oracle. OpenImageIO is optional: OIIOTOOL
#   writes more DPX variants and OIIO_PYTHON (a python with the OpenImageIO module) reads every
#   file as a second oracle, e.g. from a venv under target/:
#     python3 -m venv target/raster/venv && target/raster/venv/bin/pip install OpenImageIO numpy
#     OIIOTOOL=target/raster/venv/bin/oiiotool OIIO_PYTHON=target/raster/venv/bin/python
#   Then (an absolute path):
#     SPECULAR_RASTER_CORPUS_FILM=$PWD/tools/corpus/raster/film \
#       cargo test -p specular-film --test oracle -- --ignored --nocapture
#
# What it makes (about 90 MB):
#   files/gen-*      written by ImageMagick and oiiotool from synthetic pictures: 8/10/12/16-bit,
#                    32-bit float, packed / method A / method B, both byte orders, grey, RGB, RGBA,
#                    CbYCr 4:4:4 and CbYCrY 4:2:2, printing density; Cineon 10-bit
#   files/oiio-*     OpenImageIO-images dpx/ and cineon/ samples and OpenImageIO's dpx test-suite
#                    inputs (damaged files included; BSD-3-Clause), pinned commits
#   files/ffmpeg-*   FFmpeg FATE dpx samples and six of samples.ffmpeg.org's dpx_samples.zip
#   expect/<file>.magick.pam.gz      ImageMagick's stored values (colourspace relabelled, not
#                                    converted), 16-bit PAM
#   expect/<file>.magicksrgb.pam.gz  its YCbCr to sRGB conversion (YCbCr files)
#   expect/<file>.magicklin.pam.gz   its log to linear conversion (log files)
#   expect/<file>.oiio.pam.gz        OpenImageIO's reading (tools/corpus/raster-film-oiio.py)
#   expect/<file>.ffmpeg.pam.gz      FFmpeg's reading, 16-bit grey, RGB or RGBA as stored
#   files/gen-py-*   written by tools/corpus/raster-film-gen.py: separate R/G/B(/A) elements,
#                    RGB + alpha element, luma + colour difference, signed, user-defined 2-5
#                    channels, composite video, RLE float; Cineon 4 channels, mixed depths, signed
#   expect/<file>.truth.pam.gz       the codes the generator wrote (.truthf: floats)
#   expect/<file>.oiiof.pam.gz       OpenImageIO's float reading of signed 8/16-bit files
#
# Downloads are untrusted data: this script only copies and unzips them, never runs anything
# from them. Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RASTER_CORPUS_FILM:-$ROOT/tools/corpus/raster/film}
MAGICK=${MAGICK:-magick}
OIIOTOOL=${OIIOTOOL:-}
OIIO_PYTHON=${OIIO_PYTHON:-}
FFMPEG=${FFMPEG-$(command -v ffmpeg || true)}
MIN_FREE_GB=${MIN_FREE_GB:-8}
OIIO_IMAGES=d13e98f92b4e75ae7cd949cc5aa1df219560fb93
OIIO=07e3c3e0373551c57e7d8e01353deaa4b3646613
F="$OUT/files"
E="$OUT/expect"
DL="$OUT/download"
mkdir -p "$F" "$E" "$DL"

die() { echo "fetch-raster-film: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  (( avail_kb >= MIN_FREE_GB * 1024 * 1024 )) || die "less than ${MIN_FREE_GB} GB free; stopping"
}

get() {
  local url=$1 dest=$2
  check_space
  [[ -s "$dest" ]] && return 0
  curl -fsSL --retry 3 -o "$dest.part" "$url" || { echo "download failed: $url" >&2; return 0; }
  mv "$dest.part" "$dest"
}

# --- Downloads ---------------------------------------------------------------------------
GH=https://raw.githubusercontent.com/AcademySoftwareFoundation
for n in dpx_nuke_10bits_rgb.dpx dpx_nuke_16bits_rgba.dpx; do
  get "$GH/OpenImageIO-images/$OIIO_IMAGES/dpx/$n" "$F/oiio-$n"
done
get "$GH/OpenImageIO-images/$OIIO_IMAGES/cineon/checker.cin" "$F/oiio-checker.cin"
for n in L.dpx R.dpx stereo.dpx; do
  get "$GH/OpenImageIO/$OIIO/testsuite/dpx/ref/$n" "$F/oiio-ref-$n"
done
for n in bad-eolpad.dpx bad-packing.dpx bomb-46341.dpx crash-1chan-10bit-filled-methodA.dpx \
         crash-badusersize.dpx crash-large-origin.dpx crash-origin-overflow.dpx \
         truncated-pixels.dpx truncated-userdata.dpx; do
  get "$GH/OpenImageIO/$OIIO/testsuite/dpx/src/$n" "$F/oiio-src-$n"
done
for n in cyan.dpx lena_4x_concat.dpx lighthouse_rgb48.dpx; do
  get "https://fate-suite.ffmpeg.org/dpx/$n" "$F/ffmpeg-$n"
done
get https://samples.ffmpeg.org/image-samples/dpx_samples.zip "$DL/dpx_samples.zip"
if [[ -s "$DL/dpx_samples.zip" ]]; then
  python3 -I -c '
import sys, zipfile, os
keep = {"bigendian_8bit_alpha", "bigendian_10bit_alpha", "bigendian_12bit_alpha",
        "bigendian_16bit_noalpha", "littleendian_10bit_noalpha", "littleendian_12bit_alpha"}
z = zipfile.ZipFile(sys.argv[1])
for i in z.infolist():
    n = os.path.basename(i.filename)
    if n.lower().endswith(".dpx") and any(k in n for k in keep) and i.file_size < 32 << 20:
        open(os.path.join(sys.argv[2], "ffmpeg-zip-" + n), "wb").write(z.read(i))
' "$DL/dpx_samples.zip" "$F"
fi

# --- Generated files ---------------------------------------------------------------------
check_space
SRC="$DL/src-rgb.png"
"$MAGICK" -size 67x41 -seed 7 plasma:red-blue -depth 16 "$SRC"
"$MAGICK" "$SRC" -alpha set -channel A -fx 'i/w' +channel -depth 16 "$DL/src-rgba.png"
# (No grey DPX from ImageMagick: 7.1.2 writes grey as RGBA quads, which it and OpenImageIO
# then read inconsistently.)
for d in 8 10 12 16; do
  for en in MSB LSB; do
    "$MAGICK" "$SRC" -depth $d -endian $en "$F/gen-im-rgb-$d-$en.dpx"
    "$MAGICK" "$DL/src-rgba.png" -depth $d -endian $en "$F/gen-im-rgba-$d-$en.dpx"
    "$MAGICK" "$SRC" -set colorspace Log -depth $d -endian $en "$F/gen-im-log-$d-$en.dpx"
  done
  "$MAGICK" "$SRC" -colorspace YCbCr -depth $d "$F/gen-im-ycc444-$d.dpx"
  "$MAGICK" "$SRC" -colorspace YCbCr -sampling-factor 4:2:2 -depth $d "$F/gen-im-ycc422-$d.dpx"
done
"$MAGICK" "$SRC" -depth 10 "$F/gen-im-rgb-10.cin"
"$MAGICK" "$SRC" -colorspace gray -depth 10 "$F/gen-im-grey-10.cin"
if [[ -n "$OIIOTOOL" ]]; then
  for t in uint8 uint10 uint12 uint16 float; do
    for pk in "Filled, method A" "Filled, method B" "Packed"; do
      tag=$(echo "$pk" | tr -dc 'A-Za-z' | tail -c 8)
      for en in big little; do
        # oiiotool 3.2 writes big-endian floats in little-endian byte order.
        [[ $t == float && $en == big ]] && continue
        "$OIIOTOOL" "$SRC" -d $t --attrib dpx:Packing "$pk" --attrib oiio:Endian $en \
          -o "$F/gen-oiio-rgb-$t-$tag-$en.dpx" 2>/dev/null || true
        "$OIIOTOOL" "$DL/src-rgba.png" -d $t --attrib dpx:Packing "$pk" --attrib oiio:Endian $en \
          -o "$F/gen-oiio-rgba-$t-$tag-$en.dpx" 2>/dev/null || true
      done
    done
    "$OIIOTOOL" "$SRC" --ch R -d $t -o "$F/gen-oiio-grey-$t.dpx" 2>/dev/null || true
  done
  "$OIIOTOOL" "$SRC" -d uint10 --attrib dpx:Transfer "Printing density" \
    -o "$F/gen-oiio-log-uint10.dpx" 2>/dev/null || true
fi
# Variants no tool writes (separate colour elements, signed, user-defined, composite, RLE
# float; Cineon with 4 channels, mixed depths, signed), with the codes they hold as an oracle.
# Needs numpy: the OpenImageIO python has it.
GEN_PYTHON=${OIIO_PYTHON:-python3}
"$GEN_PYTHON" -I "$ROOT/tools/corpus/raster-film-gen.py" "$F" "$E" \
  || echo "raster-film-gen.py failed (numpy missing?); skipped" >&2

# --- Oracles (gzipped 16-bit PAM) --------------------------------------------------------
check_space
for f in "$F"/*; do
  n=$(basename "$f")
  # FFmpeg's reading at 16 bits with the file's channels (its pam encoder; YCbCr as RGB).
  if [[ -n "$FFMPEG" && ! -e "$E/$n.ffmpeg.pam.gz" ]]; then
    pf=$("${FFMPEG%ffmpeg}ffprobe" -v error -select_streams v:0 -show_entries stream=pix_fmt \
      -of csv=p=0 "$f" 2>/dev/null || true)
    case $pf in
      gray*) out=gray16be ;;
      ya*|*a|*a10*|*a12*|*a16*|rgba*|gbrap*|yuva*) out=rgba64be ;;
      *) out=rgb48be ;;
    esac
    "$FFMPEG" -v error -nostdin -i "$f" -frames:v 1 -pix_fmt $out -c:v pam -f image2pipe - \
      2>/dev/null | gzip -1 > "$E/$n.ffmpeg.pam.gz" || true
  fi
  [[ -s "$E/$n.magick.pam.gz" ]] && continue
  cs=$("$MAGICK" identify -format '%[colorspace]' "$f[0]" 2>/dev/null || true)
  # Stored values: the image's colourspace relabelled (no Log or YCbCr conversion).
  "$MAGICK" "$f[0]" -set colorspace sRGB -depth 16 PAM:- 2>/dev/null | gzip -1 \
    > "$E/$n.magick.pam.gz" || true
  # ImageMagick's conversions: YCbCr to sRGB; log to linear RGB (its printing-density curve
  # scales codes by 1024/1023, so it differs from Kodak's by up to about 0.6 %).
  if [[ $cs == *YCbCr* ]]; then
    "$MAGICK" "$f[0]" -colorspace sRGB -depth 16 PAM:- 2>/dev/null | gzip -1 \
      > "$E/$n.magicksrgb.pam.gz" || true
  fi
  if [[ $cs == Log ]]; then
    "$MAGICK" "$f[0]" -colorspace RGB -depth 16 PAM:- 2>/dev/null | gzip -1 \
      > "$E/$n.magicklin.pam.gz" || true
  fi
  if [[ -n "$OIIO_PYTHON" ]]; then
    if "$OIIO_PYTHON" -I "$ROOT/tools/corpus/raster-film-oiio.py" "$f" "$E/$n.oiio.pam" 2>/dev/null; then
      gzip -1f "$E/$n.oiio.pam"
      [[ -e "$E/$n.oiiof.pam" ]] && gzip -1f "$E/$n.oiiof.pam"
    else
      rm -f "$E/$n.oiio.pam"
    fi
  fi
done
# Empty outputs (a reader that refused the file).
find "$E" -name '*.gz' -size -30c -delete
echo "fetch-raster-film: $(ls "$F" | wc -l) files in $F, oracles in $E"
