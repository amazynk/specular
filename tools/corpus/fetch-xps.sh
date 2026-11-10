#!/usr/bin/env bash
# Fetch the public XPS / OpenXPS test corpus into tools/corpus/docs/xps/ (gitignored: nothing
# downloaded here is committed). The specular-xps oracle test reads it in place from
# SPECULAR_XPS_CORPUS, else from tools/corpus/docs/xps, and skips when it is absent.
#
# Usage: tools/corpus/fetch-xps.sh
#
# What it covers (about 45 MB): the QualityLogic "MinBar" documents of Microsoft's
# Windows-driver-samples (print/SampleXPS and print/SampleOpenXPS; MS-PL), which exercise the
# whole fixed-page markup (paths, gradients, image and visual brushes, glyphs with obfuscated
# fonts, opacity masks, resource dictionaries, JPEG/PNG/TIFF/JPEG XR images), the XPS
# conformance-violation set of the same samples (damaged and non-conforming packages that
# must fail or draw what they can, never crash), and the XPSDrv watermark vector sample.
# Everything is pinned to tag 87574 of the repository.
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_XPS_CORPUS:-$ROOT/tools/corpus/docs/xps}
BASE=https://raw.githubusercontent.com/microsoft/Windows-driver-samples/87574
MIN_FREE_GB=${MIN_FREE_GB:-6}

free_gb() { df --output=avail -BG "$ROOT" | tail -1 | tr -dc 0-9; }

get() { # get REMOTE-PATH LOCAL-NAME
    local dest="$OUT/$2"
    [ -s "$dest" ] && return 0
    if [ "$(free_gb)" -lt "$MIN_FREE_GB" ]; then
        echo "fetch-xps: less than ${MIN_FREE_GB} GB free, stopping" >&2
        exit 1
    fi
    mkdir -p "$(dirname "$dest")"
    curl -fsSL --retry 3 -o "$dest.part" "$BASE/$1"
    mv "$dest.part" "$dest"
}

for i in 1 2 3 4 5 6 7 8 9; do
    get "print/SampleXPS/QualityLogicMinBar/mb0$i.xps" "minbar/mb0$i.xps"
    # mb03 is 14 MB and the same content as the XPS one.
    [ "$i" = 3 ] || get "print/SampleOpenXPS/QualityLogicMinBar_oxps/mb0$i.oxps" "minbar/mb0$i.oxps"
done
get "print/XPSDrvSmpl/src/filters/watermark/Vector1.xps" "misc/Vector1.xps"

# The XPS conformance violations (markup rules): listed through the GitHub tree API.
list=$(curl -fsSL "https://api.github.com/repos/microsoft/Windows-driver-samples/git/trees/87574?recursive=1" |
    grep -o '"path": *"print/SampleXPS/ConformanceViolations/XPS/[^"]*\.xps"' |
    sed 's/.*"\(print[^"]*\)"/\1/')
for p in $list; do
    get "$p" "violations/$(basename "$p")"
done

echo "fetch-xps: $(find "$OUT" -type f | wc -l) files, $(du -sh "$OUT" | cut -f1) in $OUT"
