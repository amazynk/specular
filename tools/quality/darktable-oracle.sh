#!/bin/sh
# darktable-cli as a gross-error oracle for Develop.
#
# Developer machine only: never in tests or CI, never a speed comparison, nothing bundled.
# darktable-cli is not installed by this script; without it the script says so and stops.
#
#   tools/quality/darktable-oracle.sh RAW_DIR OUT_DIR [SPECULAR]
#
# RAW_DIR   raws to compare (your own; never tools/corpus/real or anyone's photos unasked)
# OUT_DIR   scratch for the renders and the report (put it under your CARGO_TARGET_DIR)
# SPECULAR  the specular binary to run (default: specular on PATH)
#
# Both sides render at default settings to 8-bit sRGB PNG (darktable: its defaults for the
# camera, sigmoid/filmic per its preferences; Specular: the default edit). The report is
# CIEDE2000 p50/p95/p99/max per raw at a common 1024 px long side. Expect differences of
# look (tone curve, highlights); the run is for gross errors: wrong white balance, wrong
# matrix, wrong orientation or crop, broken demosaic.
set -eu

if ! command -v darktable-cli >/dev/null 2>&1; then
    echo "darktable-cli is not installed: nothing to compare."
    exit 0
fi
if [ $# -lt 2 ]; then
    echo "usage: $0 RAW_DIR OUT_DIR [SPECULAR]" >&2
    exit 2
fi
raws=$1
out=$2
specular=${3:-specular}
mkdir -p "$out/darktable" "$out/specular" "$out/dt-config"

for f in "$raws"/*; do
    [ -f "$f" ] || continue
    base=$(basename "$f")
    stem=${base%.*}
    # Its own config folder: never the user's ~/.config/darktable.
    darktable-cli "$f" "$out/darktable/$stem.png" --width 1024 --height 1024 \
        --core --configdir "$out/dt-config" >/dev/null 2>&1 \
        || echo "darktable-cli failed on $base"
    "$specular" develop export "$f" --format png --depth 8 --space srgb --size 1024 \
        --metadata none --folder "$out/specular" --name "{name}" >/dev/null 2>&1 \
        || echo "specular failed on $base"
done

python3 -I "$(dirname "$0")/darktable_compare.py" "$out/darktable" "$out/specular" \
    | tee "$out/report.md"
