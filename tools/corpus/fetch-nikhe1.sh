#!/usr/bin/env bash
# Nikon High Efficiency (HE / HE*, intoPIX TicoRAW) NEF corpus for the specular-raw HE decoder
# tests, with lossless NEFs of the same scenes for comparison. Into tools/corpus/nikhe1/
# (gitignored). All files are CC0 (raw.pixls.us), checked against its sha256 file list.
#
# Usage: tools/corpus/fetch-nikhe1.sh [--adobe]
#   --adobe also writes the oracle: Adobe DNG Converter's decode of every HE / HE* file, as
#   tools/corpus/nikhe1/adobe/<body>/<name>.dng (lossless compressed CFA DNGs). It needs the
#   converter's Windows build 18.4 in a private wine prefix (see tools/corpus/adobe-dng.sh for
#   the install; env WINEPREFIX, default target/adobe-dng/wine, or CONVERTER).
# Downloads are untrusted data: this script only copies files, never runs anything from them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/tools/corpus/nikhe1"
MIN_FREE_GB=${MIN_FREE_GB:-6}
mkdir -p "$OUT"

die() { echo "fetch-nikhe1: $*" >&2; exit 1; }

# About 280 MB with the Adobe oracle. Checked but left out for size (same
# results): the Z 8 and Z 9 HE / HE* pairs, the Z 6III full-frame trio,
# the Z5II full-frame trio and the lossless Z 8, Z 9 and Z f files (other scenes).
FILES=(
  'Nikon/Z f/DSC_0042.NEF'                                                      # HE*
  'Nikon/Z f/DSC_0043.NEF'                                                      # HE
  # Same scene in HE, HE* and lossless (Z 6III 1:1 crop, Z 50II DX), and a Z5II DX crop.
  'NIKON CORPORATION/NIKON Z6_3/Nikon_Z6__3_High_Efficiency_1_1.NEF'
  'NIKON CORPORATION/NIKON Z6_3/Nikon_Z6__3_High_Efficiency_Star_1_1.NEF'
  'NIKON CORPORATION/NIKON Z6_3/Nikon_Z6__3_Lossless_compressed_1_1.NEF'
  'NIKON CORPORATION/NIKON Z50_2/DSC_0060_DX_Lossless.NEF'
  'NIKON CORPORATION/NIKON Z50_2/DSC_0061_DX_HE_Star.NEF'
  'NIKON CORPORATION/NIKON Z50_2/DSC_0062_DX_HE.NEF'
  'NIKON CORPORATION/NIKON Z5_2/High_Efficiency_DX.NEF'
)

list="$OUT/.rpu_filelist.sha256"
[[ -s $list ]] || curl -fsSL -o "$list" https://raw.pixls.us/data/filelist.sha256
for p in "${FILES[@]}"; do
  dest="$OUT/${p#Nikon/}"
  dest="${dest/NIKON CORPORATION\/NIKON /}"
  sha=$(grep -F " *$p" "$list" | head -1 | cut -d' ' -f1)
  [[ -n $sha ]] || die "not in raw.pixls.us filelist: $p"
  if [[ -s $dest ]] && echo "$sha  $dest" | sha256sum -c --status; then
    continue
  fi
  avail=$(df --output=avail -BG "$OUT" | tail -1 | tr -dc 0-9)
  (( avail >= MIN_FREE_GB )) || die "only ${avail} GB free"
  enc=$(python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$p")
  mkdir -p "$(dirname "$dest")"
  echo "fetch-nikhe1: $p"
  curl -fL --progress-bar -o "$dest.part" "https://raw.pixls.us/data/$enc"
  echo "$sha  $dest.part" | sha256sum -c --status || die "checksum mismatch: $p"
  mv "$dest.part" "$dest"
done

if [[ ${1:-} == --adobe ]]; then
  : "${WINEPREFIX:=$ROOT/target/adobe-dng/wine}"
  export WINEPREFIX WINEDEBUG=${WINEDEBUG:--all} WINEDLLOVERRIDES="winemenubuilder.exe=d;mscoree=d;mshtml=d"
  CONVERTER=${CONVERTER:-"$WINEPREFIX/drive_c/Program Files/Adobe/Adobe DNG Converter/Adobe DNG Converter.exe"}
  [[ -e $CONVERTER ]] || die "no Adobe DNG Converter at $CONVERTER (set CONVERTER or WINEPREFIX)"
  work="$ROOT/target/agent-corpus/nikhe1-adobe"
  rm -rf "$work"
  mkdir -p "$work/in" "$work/out"
  names=()
  n=0
  while IFS= read -r -d '' f; do
    rel=${f#"$OUT/"}
    [[ $rel == adobe/* ]] && continue
    [[ -s $OUT/adobe/${rel%.*}.dng ]] && continue
    c=$(exiftool -s3 -NEFCompression "$f")
    [[ $c == "High Efficiency"* ]] || continue
    ln -s "$f" "$work/in/src$n.nef"
    names+=("$rel")
    n=$((n + 1))
  done < <(find "$OUT" -name '*.NEF' -print0 | sort -z)
  if (( n )); then
    win() { winepath -w "$1"; }
    ins=()
    for ((i = 0; i < n; i++)); do ins+=("$(win "$work/in/src$i.nef")"); done
    # -c: lossless compressed raw data; -p0: medium-size preview.
    nice xvfb-run -a wine "$CONVERTER" -c -p0 -d "$(win "$work/out")" "${ins[@]}" \
      > "$work/converter.log" 2>&1 || true
    command -v wineserver > /dev/null && wineserver -w || true
    for ((i = 0; i < n; i++)); do
      [[ -s $work/out/src$i.dng ]] || die "no DNG for ${names[i]} (see $work/converter.log)"
      mkdir -p "$(dirname "$OUT/adobe/${names[i]}")"
      mv -f "$work/out/src$i.dng" "$OUT/adobe/${names[i]%.*}.dng"
    done
  fi
  rm -rf "$work"
fi
echo "fetch-nikhe1: done ($(du -sh "$OUT" | cut -f1))"
