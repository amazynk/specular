#!/usr/bin/env bash
# Regenerate the Adobe DNG Converter DNGs listed in tools/dng-colour-sources.tsv (rows with a
# fifth "source" column) from their raw.pixls.us source raws, then check that their colour
# profiles match the manifest (tools/gen-adobe-coeff.py, check mode).
#
# Usage: tools/corpus/adobe-dng.sh [PREFIX...]     (default: every converter row)
#   env: CONVERTER (converter binary), WINEPREFIX (default target/adobe-dng/wine),
#        DNG_SOURCES (manifest), ADOBE_DNG_TARGET (scratch, default target/adobe-dng)
#
# Needs the source raws (tools/corpus/fetch-conformance.sh large_raw large_raw_adobe) and Adobe
# DNG Converter (free, Adobe's licence; not redistributed here). The manifest was made with the
# Windows build 18.4 (download.adobe.com/pub/adobe/dng/win/AdobeDNGConverter_x64_18_4.exe,
# sha256 bb80cd55e49264d2b5ef39437fe7483c292be763101bc72ecbcfea7a9b48b929) under Wine 10:
#
#   export WINEPREFIX=$PWD/target/adobe-dng/wine           # private prefix, never ~/.wine
#   xvfb-run -a wineboot -i
#   xvfb-run -a wine AdobeDNGConverter_x64_18_4.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-
#
# 18.5 and later do not convert under Wine 10 (their C2PA library imports DirectML.dll). On macOS,
# point CONVERTER at ".../Adobe DNG Converter.app/Contents/MacOS/Adobe DNG Converter".
# The check also requires the converter version named in each note: another version fails it
# even when its profiles are the same (17.5, 18.2 and 18.4 wrote identical ones), so a converter
# update is a deliberate manifest edit.
#
# The converter does not write byte-identical files twice (fresh DNG UUIDs and dates), so the
# manifest pins a digest of the colour tags instead of the file hash.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
RAW="$ROOT/tools/corpus/large/raw"
TSV=${DNG_SOURCES:-"$ROOT/tools/dng-colour-sources.tsv"}
SCRATCH=${ADOBE_DNG_TARGET:-"$ROOT/target/adobe-dng"}
WORK="$SCRATCH/work"
MIN_FREE_GB=${MIN_FREE_GB:-6}
: "${WINEPREFIX:=$SCRATCH/wine}"
export WINEPREFIX WINEDEBUG=${WINEDEBUG:--all}
CONVERTER=${CONVERTER:-"$WINEPREFIX/drive_c/Program Files/Adobe/Adobe DNG Converter/Adobe DNG Converter.exe"}

die() { echo "adobe-dng: $*" >&2; exit 1; }
[[ -e $CONVERTER ]] || die "no converter at $CONVERTER (set CONVERTER or WINEPREFIX)"
avail_kb=$(df -Pk "$ROOT" | awk 'NR==2{print $4}')
(( avail_kb >= MIN_FREE_GB * 1024 * 1024 )) || die "less than ${MIN_FREE_GB} GB free"

rm -rf "$WORK"
mkdir -p "$WORK/in" "$WORK/out"
declare -a dests=()
n=0
while IFS=$'\t' read -r prefix dng _digest _note source; do
  [[ -z $prefix || $prefix == \#* || -z ${source:-} ]] && continue
  if (( $# )); then
    want=0
    for p in "$@"; do [[ $p == "$prefix" ]] && want=1; done
    (( want )) || continue
  fi
  [[ -f "$RAW/$source" ]] || die "missing source raw $source (run fetch-conformance.sh large_raw_adobe)"
  # Safe names: the converter names its output after the input, and some sources contain ':'.
  ext=${source##*.}
  ln -s "$RAW/$source" "$WORK/in/src$n.$ext"
  dests+=("$dng")
  n=$((n + 1))
done < "$TSV"
(( n )) || die "no converter rows selected"

# -c: lossless compressed raw data; -p0: medium-size JPEG preview (the converter default).
args=(-c -p0)
if [[ $CONVERTER == *.exe ]]; then
  win() { winepath -w "$1"; }
  ins=()
  for f in "$WORK"/in/*; do ins+=("$(win "$f")"); done
  nice xvfb-run -a wine "$CONVERTER" "${args[@]}" -d "$(win "$WORK/out")" "${ins[@]}" \
    > "$WORK/converter.log" 2>&1 || true
  wineserver -w || true
else
  nice "$CONVERTER" "${args[@]}" -d "$WORK/out" "$WORK"/in/* > "$WORK/converter.log" 2>&1 || true
fi

fail=0
for ((i = 0; i < n; i++)); do
  out="$WORK/out/src$i.dng"
  if [[ ! -s $out ]]; then
    echo "adobe-dng: no output for ${dests[i]} (see $WORK/converter.log)" >&2
    fail=1
    continue
  fi
  mkdir -p "$(dirname "$RAW/${dests[i]}")"
  mv -f "$out" "$RAW/${dests[i]}"
done
(( fail == 0 )) || die "some conversions failed"

python3 -I "$ROOT/tools/gen-adobe-coeff.py" --dng-sources "$TSV" --dng-root "$RAW"
rm -rf "$WORK"
