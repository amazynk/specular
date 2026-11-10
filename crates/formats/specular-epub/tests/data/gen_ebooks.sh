#!/bin/sh
# MOBI, AZW3 and FB2 fixtures (tests/ebooks.rs): one small book written as Markdown, made an
# EPUB by pandoc 3, then converted by Calibre 9's ebook-convert (used as an outside tool
# only), with Calibre's own readings as the oracle:
#   NAME.meta   ebook-meta's Title, Author(s), Publisher, Languages lines
#   NAME.txt    ebook-convert's plain-text rendering (the words to find)
# Run from this directory: sh gen_ebooks.sh
set -e
q=""
T=${TMPDIR:-.}/ebooks.$$
mkdir -p "$T"
trap 'rm -rf "$T"' EXIT
# Calibre keeps its settings beside the scratch files, not in the user's.
export CALIBRE_CONFIG_DIRECTORY="$T/calibre"
ffmpeg -nostdin -loglevel error -y -f lavfi -i testsrc2=size=120x160:rate=1:duration=1 -frames:v 1 "$T/cover.jpg"
ffmpeg -nostdin -loglevel error -y -f lavfi -i testsrc=size=64x48:rate=1:duration=1 -frames:v 1 "$T/figure.png"
cat > "$T/book.md" <<'EOF'
---
title: The Lighthouse Keeper
author: Mara Quill
publisher: Specular Fixtures
lang: en
date: 2026-05-01
---

# Arrival

The boat left her on the rocks at dawn, with a trunk and a lamp.

She climbed the *ninety-nine* steps and found the **logbook** open.

# Storm

Wind came over the cape before noon. The glass fell all afternoon.

![The beam](figure.png)

## The Lamp

She trimmed the wick twice before midnight.

# Morning

Gulls, salt, and a calm grey sea.
EOF
(cd "$T" && pandoc book.md -o book.epub --epub-cover-image=cover.jpg --toc)
ebook-convert "$T/book.epub" ebook-mobi6.mobi --mobi-file-type old $q >/dev/null
ebook-convert "$T/book.epub" ebook-joint.mobi --mobi-file-type both $q >/dev/null
ebook-convert "$T/book.epub" ebook-kf8.azw3 $q >/dev/null
# FB2 by pandoc (Calibre writes no section titles).
(cd "$T" && pandoc book.md -o book.fb2 --metadata cover-image=cover.jpg) && cp "$T/book.fb2" ebook.fb2
# FB2 zipped alone.
(cp ebook.fb2 "$T/ebook.fb2" && cd "$T" && zip -q -X ebook.fb2.zip ebook.fb2) && cp "$T/ebook.fb2.zip" ebook-zip.fb2.zip
for f in ebook-mobi6.mobi ebook-joint.mobi ebook-kf8.azw3 ebook.fb2; do
    stem=${f%.*}
    ebook-meta "$f" | grep -E '^(Title|Author\(s\)|Publisher|Languages) ' | sed 's/  *: /: /' > "$stem.meta"
    ebook-convert "$f" "$T/$stem.txt" $q >/dev/null && cp "$T/$stem.txt" "$stem.txt"
done
