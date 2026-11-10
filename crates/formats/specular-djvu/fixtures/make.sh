#!/bin/sh
# Builds the DjVu test fixtures from the synthetic images (DjVuLibre tools as encoders).
set -e
# DjVuLibre tools on PATH, or DJ set to a wrapper that runs them.
DJ=${DJ:-}
cd "$(dirname "$0")"
python3 mkimg.py .
convert -size 640x400 xc:white -font DejaVu-Sans -pointsize 28 -fill black -annotate +20+60 "DjVu JB2 test: the quick brown fox" -annotate +20+120 "jumps over the lazy dog 0123456789" -annotate +20+180 "Mmm aaa eee ooo repeated shapes" -draw "rectangle 20,300 300,320" -monochrome text.pbm
$DJ c44 -slice 74+13+10 photo.ppm photo.djvu
$DJ c44 -crcbfull photo.ppm photo_full.djvu
$DJ c44 gray.pgm gray.djvu
$DJ cjb2 -lossy text.pbm text.djvu
$DJ cpaldjvu compound.ppm compound.djvu
$DJ cjb2 text.pbm text_refine.djvu
cp text.djvu text_rot.djvu
$DJ djvused -e 'select 1; set-rotation 1' -s text_rot.djvu
cp photo.djvu photo_rot90.djvu
$DJ djvused -e 'select 1; set-rotation 1' -s photo_rot90.djvu
cp photo.djvu photo_rot180.djvu
$DJ djvused -e 'select 1; set-rotation 2' -s photo_rot180.djvu
cp photo.djvu photo_rot270.djvu
$DJ djvused -e 'select 1; set-rotation 3' -s photo_rot270.djvu
cat > txt.sx <<'EOF'
(page 0 0 640 400
 (line 20 330 560 370 (word 20 335 90 370 "DjVu") (word 100 335 160 370 "JB2") (word 170 335 240 370 "test:"))
 (line 20 270 520 310 (word 20 275 120 310 "jumps") (word 130 275 210 310 "over")))
EOF
cp text.djvu text_txt.djvu
$DJ djvused -e 'select 1; set-txt txt.sx' -s text_txt.djvu
cat > meta.sx <<'EOF'
Title	"Fixture document"
Author	"Specular tests"
Producer	"DjVuLibre djvm"
EOF
$DJ djvm -c doc.djvu text_txt.djvu photo.djvu gray.djvu text_refine.djvu
$DJ djvused -e 'set-meta meta.sx' -s doc.djvu
$DJ djvused -e 'set-thumbnails 48' -s doc.djvu
$DJ djvused -e "set-outline outline.sx" -s doc.djvu
rm -rf indirect && mkdir indirect
$DJ djvmcvt -i doc.djvu indirect index.djvu
ls -la *.djvu indirect
# A G4 (Smmr) mask page: ImageMagick's Group 4 TIFF strip wrapped by mmr.py.
convert text.pbm -compress Group4 -define tiff:rows-per-strip=400 g4.tif
python3 mmr.py g4.tif mmr.bin
$DJ djvumake mmr.djvu INFO=640,400,300 Smmr=mmr.bin
rm -f g4.tif mmr.bin
rm -f *.ppm *.pgm *.pbm txt.sx meta.sx
