#!/bin/sh
# Downloads a small real-font corpus (about 40 MB, OFL/Apache/CC licences) into
# tools/corpus/fonts/ for the specular-font corpus oracle test:
#   cargo test -p specular-font --test corpus -- --nocapture
set -eu
dir="$(cd "$(dirname "$0")" && pwd)/fonts"
mkdir -p "$dir"
cd "$dir"

get() {
	[ -s "$1" ] || curl -fsSL --retry 3 -o "$1" "$2" || { rm -f "$1"; echo "skipped $1" >&2; }
}

gf=https://raw.githubusercontent.com/google/fonts/main
ss=https://raw.githubusercontent.com/adobe-fonts/source-sans/release
twemoji=https://github.com/mozilla/twemoji-colr/releases/download/v0.7.0/Twemoji.Mozilla.ttf
# TrueType, variable TrueType (fvar, gvar, avar, STAT).
get Inter-Variable.ttf "$gf/ofl/inter/Inter%5Bopsz,wght%5D.ttf"
get NotoSans-Variable.ttf "$gf/ofl/notosans/NotoSans%5Bwdth,wght%5D.ttf"
get Lobster-Regular.ttf "$gf/ofl/lobster/Lobster-Regular.ttf"
get RobotoMono-Variable.ttf "$gf/ofl/robotomono/RobotoMono%5Bwght%5D.ttf"
# CFF, CFF2 variable, WOFF and WOFF2.
get SourceSans3-Regular.otf "$ss/OTF/SourceSans3-Regular.otf"
get SourceSans3VF-Upright.otf "$ss/VF/SourceSans3VF-Upright.otf"
get SourceSans3-Regular.otf.woff "$ss/WOFF/OTF/SourceSans3-Regular.otf.woff"
get SourceSans3-Regular.ttf.woff2 "$ss/WOFF2/TTF/SourceSans3-Regular.ttf.woff2"
get SourceSans3-It.otf.woff2 "$ss/WOFF2/OTF/SourceSans3-It.otf.woff2"
get SourceSans3VF-Upright.ttf.woff2 "$ss/WOFF2/VF/SourceSans3VF-Upright.ttf.woff2"
# Colour: COLR v1 (one of them variable), COLR v0 and CBDT.
get BungeeSpice-Regular.ttf "$gf/ofl/bungeespice/BungeeSpice-Regular.ttf"
get Nabla-Variable.ttf "$gf/ofl/nabla/Nabla%5BEDPT,EHLT%5D.ttf"
get Twemoji.Mozilla.ttf "$twemoji"
get NotoColorEmoji-Regular.ttf "$gf/ofl/notocoloremoji/NotoColorEmoji-Regular.ttf"
ls -l "$dir"
