#!/bin/sh
# Public EPUB books for specular-epub's oracle and timing runs (about 38 MB), into
# tools/corpus/epub/ (gitignored). IDPF epub3-samples (CC BY-SA / various, see each book) and
# Project Gutenberg books (public domain in the USA).
#
#   sh tools/corpus/fetch-epub.sh
#   venv/bin/python -I tools/corpus/epub-oracle.py tools/corpus/epub crates/formats/specular-epub/fixtures
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/epub"
mkdir -p "$out"
get() {
    [ -s "$out/$2" ] || curl -fsSL --retry 3 --max-time 300 -o "$out/$2" "$1"
}
samples=https://github.com/IDPF/epub3-samples/releases/download/20230704
for f in wasteland-otf-obf wasteland-woff-obf page-blanche haruko-html-jpeg \
    regime-anticancer-arabic childrens-literature sous-le-vent georgia-cfi moby-dick \
    israelsailing; do
    get "$samples/$f.epub" "idpf-$f.epub"
done
pg=https://www.gutenberg.org/ebooks
get "$pg/2701.epub.noimages" pg-2701-moby-dick-epub2.epub
get "$pg/2600.epub.noimages" pg-2600-war-and-peace-epub2.epub
get "$pg/1342.epub3.images" pg-1342-pride-and-prejudice-epub3.epub
get "$pg/11.epub3.images" pg-11-alice-epub3.epub
get "$pg/84.epub3.images" pg-84-frankenstein-epub3.epub
get "$pg/1661.epub.noimages" pg-1661-sherlock-epub2.epub
du -sh "$out"
