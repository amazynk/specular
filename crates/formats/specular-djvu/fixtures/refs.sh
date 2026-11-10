#!/bin/sh
# Renders the fixtures with DjVuLibre's ddjvu (full size) into PNG references.
set -e
# DjVuLibre tools on PATH, or DJ set to a wrapper that runs them.
DJ=${DJ:-}
cd "$(dirname "$0")"
mkdir -p ref
for f in photo photo_full gray text text_refine text_rot photo_rot90 photo_rot180 photo_rot270 mmr; do
  $DJ ddjvu -format=ppm "$f.djvu" "ref/$f.ppm"
done
for p in 1 2 3 4; do
  $DJ ddjvu -format=ppm -page=$p doc.djvu "ref/doc_p$p.ppm"
done
mogrify -format png ref/*.ppm
rm ref/*.ppm
ls -la ref
