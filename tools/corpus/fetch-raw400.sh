#!/usr/bin/env bash
# Fetch the corpus of old and rare camera RAW containers: Minolta RD175 (.mdc), Apple QuickTake
# (.qtk), Kodak DC25 (.k25), Sinar (.sti), Olympus high-resolution (.ori), old Panasonic, Leica,
# Kodak and action-camera .raw files, Ricoh (.j6i), FlashPix (.fpx) and Lytro (.lfr/.lfp).
# Into tools/corpus/raw400/ (gitignored: nothing here is committed).
#
# Sources: raw.pixls.us (CC0), telparia.com fileFormatSamples (sample files collected from old
# software and discs; licence varies, local testing only). Pinned by URL and SHA-256.
#
# Usage: tools/corpus/fetch-raw400.sh
#   OUT defaults to $SPECULAR_RAW400_CORPUS, else tools/corpus/raw400.
# Then: SPECULAR_RAW400_CORPUS=tools/corpus/raw400 cargo test -p specular-raw --test raw400 -- --ignored
#
# Downloads are untrusted data: this script only copies them. Stops if free space drops below
# MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RAW400_CORPUS:-$ROOT/tools/corpus/raw400}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-raw400: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get SHA256 URL REL LICENCE: download URL to $OUT/REL with a progress bar, then check SHA-256
# ("-" prints it instead).
get() {
  local sum=$1 url=$2 rel=$3 lic=$4 out="$OUT/$3"
  mkdir -p "$(dirname "$out")"
  if [[ ! -s $out ]]; then
    check_space
    echo "fetch-raw400: downloading $rel"
    curl -fL --progress-bar --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" "$url" \
      || { rm -f "$out.part"; die "download failed: $url"; }
    mv "$out.part" "$out"
  fi
  local got
  got=$(sha256sum "$out" | cut -d' ' -f1)
  if [[ $sum == - ]]; then
    echo "$got $rel"
  elif [[ $got != "$sum" ]]; then
    rm -f "$out"
    die "$rel: SHA-256 $got, expected $sum"
  fi
  printf '%s\t%s\t%s\n' "$rel" "$url" "$lic" >> "$OUT/.provenance.tsv"
}

enc() { python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$1"; }

pixls() { get "$1" "https://raw.pixls.us/data/$(enc "$2")" "pixls/$2" 'CC0-1.0 (raw.pixls.us)'; }
# telparia FOLDER FILE: FILE as listed (already URL-encoded) in the folder's index.
telparia() {
  get "$1" "https://telparia.com/fileFormatSamples/image/$2/$3" "telparia/$2/$(python3 -I -c 'import sys,urllib.parse;s=sys.argv[1]
for _ in range(4): s=urllib.parse.unquote(s)
print(s.replace("/","_"))' "$3")" 'telparia.com fileFormatSamples (local testing only)'
}

LIST=${RAW400_LIST:-$ROOT/tools/corpus/raw400.tsv}
while IFS=$'\t' read -r kind sum a b; do
  [[ -z $kind || $kind == \#* ]] && continue
  case $kind in
    pixls) pixls "$sum" "$a" ;;
    telparia) telparia "$sum" "$a" "$b" ;;
    url) get "$sum" "$a" "$b" 'see URL' ;;
    # zip SHA URL ZIPNAME: its files are unpacked beside it (no code runs), then the archive
    # is deleted; a `.unpacked` marker skips it next time.
    zip)
      [[ -e "$OUT/$b.unpacked" ]] && continue
      get "$sum" "$a" "$b" 'see URL'
      python3 -I -c 'import sys,zipfile,os
z=zipfile.ZipFile(sys.argv[1]); d=os.path.dirname(sys.argv[1])
for n in z.namelist():
    b=os.path.basename(n)
    if b and not b.startswith(".") and not os.path.exists(os.path.join(d,b)):
        open(os.path.join(d,b),"wb").write(z.read(n))' "$OUT/$b"
      rm -f "$OUT/$b"
      touch "$OUT/$b.unpacked"
      ;;
    *) die "unknown source $kind" ;;
  esac
done < "$LIST"
echo "fetch-raw400: $(du -sh "$OUT" | cut -f1) in $OUT"
