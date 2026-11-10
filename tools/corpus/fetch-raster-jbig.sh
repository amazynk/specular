#!/usr/bin/env bash
# Build the JBIG1 (T.82) test corpus into tools/corpus/raster/jbig/ (gitignored: nothing here is
# committed) and the oracle tools that make and check it.
#
# Usage: tools/corpus/fetch-raster-jbig.sh
#   Then: SPECULAR_RASTER_CORPUS_JBIG=tools/corpus/raster/jbig/files \
#         cargo test -p specular-jbig --test oracle -- --ignored --nocapture
#
# Oracle: JBIG-KIT 2.1 (Markus Kuhn; GPL-2.0+) built from its release tarball into bin/ as a
# separate program. It is a test tool only: nothing of it is linked into Specular or copied into
# its code. pbmtojbg writes every variant (layers, stripe orders, TP/DP on and off, two- and
# three-line templates, adaptive-template moves, SDRST, NEWLEN, comments, several bit planes,
# T.85 fax options); jbgtopbm decodes them. JBIG is lossless, so the expected image is the source
# PBM/PGM itself; jbgtopbm's decode is checked against it too.
# Sources: ImageMagick's built-in images (rose:, logo:, wizard:, granite:, netscape:), dithered or
# thresholded to bi-level at several sizes, plus synthetic patterns. The ImageMagick JBIG writer
# (through its own JBIG-KIT) adds a few more files.
#
# The tarball is untrusted data: it is unpacked in its own directory and compiled with the
# system C compiler by naming its source files (its Makefiles are not run). Stops if free
# space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RASTER_CORPUS_JBIG_ROOT:-$ROOT/tools/corpus/raster/jbig}
MIN_FREE_GB=${MIN_FREE_GB:-8}
URL=https://www.cl.cam.ac.uk/~mgk25/jbigkit/download/jbigkit-2.1.tar.gz
SHA=de7106b6bfaf495d6865c7dd7ac6ca1381bd12e0d81405ea81e7f2167263d932
F=$OUT/files
SRC=$OUT/src
BIN=$OUT/bin
SEED=$OUT/seed
mkdir -p "$F" "$SRC" "$BIN" "$SEED"

die() { echo "fetch-raster-jbig: $*" >&2; exit 1; }
avail=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
(( avail >= MIN_FREE_GB * 1024 * 1024 )) || die "less than ${MIN_FREE_GB} GB free"
command -v magick >/dev/null || die "ImageMagick (magick) is needed"
command -v cc >/dev/null || die "a C compiler (cc) is needed to build the JBIG-KIT oracle"

if [[ ! -x $BIN/pbmtojbg || ! -x $BIN/jbgtopbm ]]; then
  [[ -s $SRC/jbigkit-2.1.tar.gz ]] || curl -fsSL --retry 3 -o "$SRC/jbigkit-2.1.tar.gz" "$URL"
  echo "$SHA  $SRC/jbigkit-2.1.tar.gz" | sha256sum -c --quiet || die "checksum mismatch"
  tar -xzf "$SRC/jbigkit-2.1.tar.gz" -C "$SRC"
  K=$SRC/jbigkit-2.1
  cc -O2 -w -I"$K/libjbig" -o "$BIN/pbmtojbg" "$K/pbmtools/pbmtojbg.c" "$K/libjbig/jbig.c" "$K/libjbig/jbig_ar.c"
  cc -O2 -w -I"$K/libjbig" -o "$BIN/jbgtopbm" "$K/pbmtools/jbgtopbm.c" "$K/libjbig/jbig.c" "$K/libjbig/jbig_ar.c"
fi
ENC=$BIN/pbmtojbg
DEC=$BIN/jbgtopbm

# Bi-level seeds (PBM) and grey seeds (PGM).
seed() { [[ -s $SEED/$1 ]] || magick "${@:2}" "$SEED/$1"; }
seed rose_fs.pbm rose: -resize 300% -colorspace gray -dither FloydSteinberg -monochrome
seed logo_th.pbm logo: -colorspace gray -threshold 60%
seed wizard_od.pbm wizard: -colorspace gray -ordered-dither o8x8 -monochrome
seed granite_ht.pbm granite: -resize 400% -colorspace gray -ordered-dither h6x6a -monochrome
seed netscape.pbm netscape: -resize 500% -colorspace gray -threshold 50%
seed odd_1x1.pbm -size 1x1 xc:black
seed odd_3x7.pbm -size 3x7 pattern:checkerboard -threshold 50%
seed wide.pbm -size 2481x37 pattern:hexagons -threshold 50%
seed tall.pbm -size 29x1311 pattern:verticalsaw -threshold 50%
seed blank.pbm -size 1728x2200 xc:white
seed page.pbm logo: -resize 1728x2376! -colorspace gray -threshold 55%
seed rose_g4.pgm rose: -resize 200% -colorspace gray -depth 4
seed wizard_g8.pgm wizard: -colorspace gray -depth 8
seed grad_g2.pgm -size 97x61 gradient: -colorspace gray -depth 2
seed grad_g12.pgm -size 64x48 gradient: -colorspace gray -depth 12

n=0
mk() { # NAME SEED ENCODER-OPTIONS...
  local name=$1 src=$2; shift 2
  [[ -s $F/$name.jbg ]] || "$ENC" "$@" "$SEED/$src" "$F/$name.jbg" 2>/dev/null || { echo "skip $name" >&2; return 0; }
  n=$((n + 1))
}
for s in rose_fs logo_th wizard_od granite_ht netscape odd_1x1 odd_3x7 wide tall; do
  mk "${s}_default" $s.pbm
  mk "${s}_seq" $s.pbm -q
  mk "${s}_two" $s.pbm -q -p 92
  mk "${s}_d3" $s.pbm -d 3
  mk "${s}_d5_s2" $s.pbm -d 5 -s 2
  for o in 0 2 3 4 5 6 8 10 11 12 13 14; do mk "${s}_d2_o$o" $s.pbm -d 2 -s 7 -o $o; done
  for p in 0 4 8 16 20 24 28 64 68 72 80 84 88 92; do mk "${s}_d2_p$p" $s.pbm -d 2 -s 5 -p $p; done
  mk "${s}_at0" $s.pbm -q -m 0
  mk "${s}_at127" $s.pbm -d 1 -m 127
  mk "${s}_at_c" $s.pbm -d 1 -m 30 -c -s 9
  mk "${s}_rst" $s.pbm -d 2 -s 3 -r
  mk "${s}_newlen" $s.pbm -q -Y 99999 -s 8
  mk "${s}_fax" $s.pbm -f
  mk "${s}_comment" $s.pbm -C "Specular JBIG test $s"
done
for s in blank page; do
  mk "${s}_default" $s.pbm
  mk "${s}_fax" $s.pbm -f
  mk "${s}_seq_two" $s.pbm -q -p 72
done
for s in rose_g4 wizard_g8 grad_g2 grad_g12; do
  mk "${s}_default" $s.pgm
  mk "${s}_seq" $s.pgm -q
  for o in 0 2 3 4 5 6; do mk "${s}_d2_o$o" $s.pgm -d 2 -s 4 -o $o; done
  mk "${s}_d1_p0" $s.pgm -d 1 -p 0
done
# ImageMagick's own writer.
for s in rose_fs logo_th; do
  [[ -s $F/${s}_magick.jbg ]] || magick "$SEED/$s.pbm" "JBIG:$F/${s}_magick.jbg"
done
# Refused variant: a BIE starting above layer 0 (DL > 0).
[[ -s $F/refuse_dl1.jbg ]] || "$ENC" -d 2 -l 1 "$SEED/logo_th.pbm" "$F/refuse_dl1.jbg"

# Expected images: the seed each file was made from (lossless), as recorded here, and
# JBIG-KIT's decode for comparison.
: > "$OUT/expect.tsv"
for f in "$F"/*.jbg; do
  b=$(basename "$f" .jbg)
  case $b in
    refuse_*) continue ;;
    *_magick) src=${b%_magick}.pbm ;;
    *)
      # The longest seed name that prefixes the file's name.
      src=
      for s in "$SEED"/*; do
        s=$(basename "$s")
        if [[ $b == "${s%.*}"_* && ${#s} -gt ${#src} ]]; then src=$s; fi
      done
      ;;
  esac
  [[ -n $src ]] || { echo "no seed for $b" >&2; continue; }
  ext=${src##*.}
  "$DEC" "$f" "$F/$b.jbigkit.$ext" 2>/dev/null || echo "jbigkit cannot decode $b" >&2
  printf '%s\t%s\n' "$b.jbg" "../seed/$src" >> "$OUT/expect.tsv"
done
echo "fetch-raster-jbig: $(ls "$F"/*.jbg | wc -l) files in $F"
