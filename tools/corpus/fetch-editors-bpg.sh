#!/usr/bin/env bash
# Build the BPG test corpus in tools/corpus/editors/bpg (gitignored; nothing here is committed).
# The specular-bpg oracle test reads it from there (or $SPECULAR_BPG_CORPUS) and skips when it
# is absent.
#
# Usage: tools/corpus/fetch-editors-bpg.sh [SECTION...]
#   SECTION is a function name below (oracle, samples, generate, refs). Default: all, in order.
#
# * oracle: libbpg 0.9.8 (Fabrice Bellard; BSD/LGPL) built as oracle-only binaries under
#   $CARGO_TARGET_DIR/oracle/libbpg (default target/oracle): bpgenc (JCTVC encoder, no x265), bpgdec, and
#   bpgframes, a small program over libbpg that writes every frame as RGBA PAM with its
#   duration. A second bpgenc with the JCTVC high-bit-depth option makes 14-bit files.
#   Nothing of libbpg is linked into Specular.
# * samples: the BPG samples of sembiance.com's file format collection.
# * generate: files bpgenc encodes from ImageMagick-made pictures: every pixel format, colour
#   space, bit depth 8-14, limited range, alpha (straight and premultiplied), CMYK, lossless,
#   animations with a frame rate and with per-frame delays, metadata (EXIF, ICC, XMP).
# * refs: refs/<name>.fNNN.pam (8-bit RGBA, or 16-bit RGBA above 8 bits) and refs/<name>.txt
#   (one "index num den" line per frame: the frame's duration in seconds as num/den).
#
# Needs curl, make, gcc, g++, ImageMagick (convert), exiftool. Downloads are untrusted data:
# this script only copies them and decodes them with the oracle. Stops if free space drops
# below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_BPG_CORPUS:-$ROOT/tools/corpus/editors/bpg}
# Oracle builds go beside the cargo target directory (CARGO_TARGET_DIR).
ORACLE=${BPG_ORACLE:-${CARGO_TARGET_DIR:-$ROOT/target}/oracle}
MIN_FREE_GB=${MIN_FREE_GB:-6}
mkdir -p "$OUT/src" "$OUT/gen" "$OUT/refs" "$ORACLE"

die() { echo "fetch-editors-bpg: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

LIBBPG="$ORACLE/libbpg"
LIBBPG_HI="$ORACLE/libbpg-hi"
BPG_MAKE=(USE_X265= USE_JCTVC=y USE_BPGVIEW=
  "CC=gcc -std=gnu99 -Wno-implicit-function-declaration -include string.h -include strings.h"
  "CXX=g++ -std=gnu++98 -fpermissive -w")

oracle() {
  check_space
  local tgz="$ORACLE/libbpg-0.9.8.tar.gz"
  [[ -f $tgz ]] || curl -fsSL -o "$tgz" https://bellard.org/bpg/libbpg-0.9.8.tar.gz
  for dir in "$LIBBPG" "$LIBBPG_HI"; do
    if [[ ! -x $dir/bpgenc ]]; then
      rm -rf "$dir" "$ORACLE/libbpg-0.9.8"
      tar -xzf "$tgz" -C "$ORACLE"
      mv "$ORACLE/libbpg-0.9.8" "$dir"
      local extra=()
      [[ $dir == "$LIBBPG_HI" ]] && extra=(USE_JCTVC_HIGH_BIT_DEPTH=y)
      nice make -C "$dir" -j2 "${BPG_MAKE[@]}" "${extra[@]}" bpgdec bpgenc >"$dir/build.log" 2>&1 ||
        die "libbpg build failed, see $dir/build.log"
    fi
  done
  # bpgframes: every frame of a BPG as RGBA PAM, with its duration.
  cat >"$LIBBPG/bpgframes.c" <<'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include "libbpg.h"
int main(int argc, char **argv) {
    if (argc < 3) { fprintf(stderr, "usage: bpgframes in.bpg outprefix\n"); return 2; }
    FILE *f = fopen(argv[1], "rb");
    if (!f) return 1;
    fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET);
    uint8_t *buf = malloc(n);
    if (fread(buf, 1, n, f) != (size_t)n) return 1;
    fclose(f);
    BPGDecoderContext *d = bpg_decoder_open();
    if (bpg_decoder_decode(d, buf, n) < 0) { fprintf(stderr, "decode failed\n"); return 1; }
    BPGImageInfo info;
    bpg_decoder_get_info(d, &info);
    int wide = info.bit_depth > 8;
    BPGDecoderOutputFormat fmt = wide ? BPG_OUTPUT_FORMAT_RGBA64 : BPG_OUTPUT_FORMAT_RGBA32;
    size_t row_bytes = (size_t)info.width * 4 * (wide ? 2 : 1);
    uint8_t *row = malloc(row_bytes);
    for (int i = 0; bpg_decoder_start(d, fmt) == 0; i++) {
        int num, den;
        bpg_decoder_get_frame_duration(d, &num, &den);
        printf("%d %d %d\n", i, num, den);
        char name[4096];
        snprintf(name, sizeof name, "%s.f%03d.pam", argv[2], i);
        FILE *o = fopen(name, "wb");
        fprintf(o, "P7\nWIDTH %u\nHEIGHT %u\nDEPTH 4\nMAXVAL %d\nTUPLTYPE RGB_ALPHA\nENDHDR\n",
                info.width, info.height, wide ? 65535 : 255);
        for (unsigned y = 0; y < info.height; y++) {
            bpg_decoder_get_line(d, row);
            if (wide) {
                uint16_t *p = (uint16_t *)row;
                for (size_t k = 0; k < (size_t)info.width * 4; k++) {
                    fputc(p[k] >> 8, o);
                    fputc(p[k] & 255, o);
                }
            } else {
                fwrite(row, 1, row_bytes, o);
            }
        }
        fclose(o);
        if (!info.has_animation) break;
    }
    bpg_decoder_close(d);
    return 0;
}
EOF
  (cd "$LIBBPG" && gcc -O1 -std=gnu99 -I. -o bpgframes bpgframes.c libbpg.a -lm) ||
    die "bpgframes build failed"
}

samples() {
  check_space
  local base=https://sembiance.com/fileFormatSamples/image/bpg
  for f in animated.bpg clock.bpg pyani.bpg test.bpg; do
    [[ -f $OUT/src/$f ]] || curl -fsSL -o "$OUT/src/$f" "$base/$f"
  done
}

generate() {
  check_space
  local enc="$LIBBPG/bpgenc" enchi="$LIBBPG_HI/bpgenc"
  local w="$OUT/gen/work"
  mkdir -p "$w"
  # Pictures: a photo-like fractal (odd size for chroma edges), ImageMagick's logo, a
  # 16-bit gradient, a grey picture, one with alpha, a CMYK JPEG, and a 2048x1536 bench
  # picture.
  convert -seed 7 -size 333x211 plasma:fractal -depth 8 "$w/photo.png"
  convert logo: -resize 50% -depth 8 "$w/logo.png"
  convert -size 257x129 gradient:'#102030-#f0c080' -depth 16 "$w/grad16.png"
  convert "$w/photo.png" -colorspace Gray -type Grayscale "$w/grey.png"
  convert "$w/photo.png" \( -size 333x211 radial-gradient:white-black \) -alpha off \
    -compose CopyOpacity -composite "$w/alpha.png"
  convert "$w/photo.png" -colorspace CMYK -quality 95 "$w/cmyk.jpg"
  convert -seed 3 -size 2048x1536 plasma:fractal -depth 8 "$w/big.png"
  # Metadata: EXIF and XMP in a JPEG, an ICC profile in a PNG.
  convert "$w/photo.png" -quality 95 "$w/meta.jpg"
  exiftool -q -overwrite_original -Artist=Specular -ImageDescription="BPG oracle" \
    -XMP-dc:Title="BPG oracle" "$w/meta.jpg"
  convert "$w/photo.png" -profile /usr/share/color/icc/DisplayP3Compat-v4.icc "$w/icc.png" ||
    cp "$w/photo.png" "$w/icc.png"

  local g="$OUT/gen"
  for f in 420 422 444; do
    "$enc" -f "$f" -o "$g/photo-$f.bpg" "$w/photo.png"
  done
  for c in ycbcr rgb ycgco ycbcr_bt709 ycbcr_bt2020; do
    "$enc" -f 444 -c "$c" -o "$g/logo-$c-444.bpg" "$w/logo.png"
    "$enc" -f 420 -c "$c" -o "$g/logo-$c-420.bpg" "$w/logo.png"
  done
  "$enc" -o "$g/grey.bpg" "$w/grey.png"
  "$enc" -b 10 -o "$g/grey-10.bpg" "$w/grey.png"
  for b in 9 10 12; do
    "$enc" -b "$b" -f 420 -o "$g/grad-$b-420.bpg" "$w/grad16.png"
    "$enc" -b "$b" -f 444 -c ycgco -o "$g/grad-$b-ycgco.bpg" "$w/grad16.png"
  done
  "$enchi" -b 14 -f 444 -o "$g/grad-14-444.bpg" "$w/grad16.png" || true
  "$enchi" -b 14 -lossless -o "$g/grad-14-lossless.bpg" "$w/grad16.png" || true
  # MPEG-2 chroma siting: the same streams with pixel format 4 (4:2:0) and 5 (4:2:2) in the
  # header (bpgenc only writes them for video input); only the chroma interpolation differs.
  python3 -I -c 'import sys
for src, dst, fmt in ((sys.argv[1], sys.argv[2], 4), (sys.argv[3], sys.argv[4], 5)):
    d = bytearray(open(src, "rb").read())
    d[4] = (d[4] & 0x1f) | fmt << 5
    open(dst, "wb").write(d)' "$g/photo-420.bpg" "$g/photo-420video.bpg" \
    "$g/photo-422.bpg" "$g/photo-422video.bpg"
  "$enc" -limitedrange -o "$g/photo-limited.bpg" "$w/photo.png"
  "$enc" -limitedrange -c ycbcr_bt709 -f 422 -b 10 -o "$g/photo-limited-709-10.bpg" "$w/photo.png"
  "$enc" -limitedrange -c rgb -f 444 -o "$g/photo-limited-rgb.bpg" "$w/photo.png"
  "$enc" -limitedrange -c ycgco -f 444 -o "$g/photo-limited-ycgco.bpg" "$w/photo.png"
  "$enc" -limitedrange -o "$g/grey-limited.bpg" "$w/grey.png"
  "$enc" -o "$g/alpha.bpg" "$w/alpha.png"
  "$enc" -premul -o "$g/alpha-premul.bpg" "$w/alpha.png"
  "$enc" -premul -b 10 -f 444 -o "$g/alpha-premul-10.bpg" "$w/alpha.png"
  "$enc" -alphaq 10 -q 35 -o "$g/alpha-q.bpg" "$w/alpha.png"
  "$enc" -o "$g/cmyk.bpg" "$w/cmyk.jpg"
  "$enc" -lossless -o "$g/photo-lossless.bpg" "$w/photo.png"
  "$enc" -lossless -f 444 -c rgb -o "$g/logo-lossless-rgb.bpg" "$w/logo.png"
  "$enc" -lossless -o "$g/alpha-lossless.bpg" "$w/alpha.png"
  "$enc" -keepmetadata -o "$g/meta-exif.bpg" "$w/meta.jpg"
  "$enc" -keepmetadata -o "$g/meta-icc.bpg" "$w/icc.png"
  "$enc" -m 1 -q 30 -o "$g/big.bpg" "$w/big.png"
  # Animations: 6 frames turning; one at 10 fps looping 3 times, one with per-frame delays
  # and alpha, one 10-bit.
  for i in 0 1 2 3 4 5; do
    convert "$w/photo.png" -resize 160x100! -distort SRT "$((i * 15))" "$w/anim_$i.png"
    convert "$w/alpha.png" -resize 160x100! -distort SRT "$((i * 15))" "$w/anima_$i.png"
  done
  printf '10\n20\n30\n10\n50\n5\n' >"$w/delays.txt"
  "$enc" -a -fps 10 -loop 3 -o "$g/anim-10fps.bpg" "$w/anim_%d.png"
  "$enc" -a -delayfile "$w/delays.txt" -o "$g/anim-alpha-delays.bpg" "$w/anima_%d.png"
  "$enc" -a -b 10 -f 444 -fps 25 -o "$g/anim-10bit.bpg" "$w/anim_%d.png"
  rm -rf "$w"
}

refs() {
  check_space
  rm -f "$OUT/refs/"*
  local f name
  for f in "$OUT"/src/*.bpg "$OUT"/gen/*.bpg; do
    [[ -f $f ]] || continue
    name=$(basename "$f" .bpg)
    "$LIBBPG/bpgframes" "$f" "$OUT/refs/$name" >"$OUT/refs/$name.txt" ||
      echo "fetch-editors-bpg: libbpg cannot decode $f" >&2
  done
}

sections=("$@")
[[ ${#sections[@]} -gt 0 ]] || sections=(oracle samples generate refs)
for s in "${sections[@]}"; do
  "$s"
done
echo "fetch-editors-bpg: corpus in $OUT"
