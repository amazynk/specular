#!/usr/bin/env bash
# Fetch the electronics and lab-spectra test corpus into tools/corpus/pcb-spectra/ (gitignored:
# nothing here is committed): Gerber/Excellon sets, KiCad boards and schematics, JCAMP-DX, SPC,
# mzML/mzXML and Bruker NMR files.
#
# Usage: tools/corpus/fetch-pcb-spectra.sh [PART...]
#   PART is gerber, kicad, jcamp, mzml or nmr (default: all). Each part's files are listed in
#   tools/corpus/pcb-spectra-PART.tsv: URL, SHA-256, destination (relative to OUT/PART),
#   licence, tab-separated; '#' starts a comment. Every file is checked against its SHA-256;
#   a mismatch is an error and the file is removed. A destination ending in / takes a .zip or
#   .tar.gz URL and unpacks it there (the archive's own SHA-256 is checked first).
#   OUT defaults to $SPECULAR_PCB_SPECTRA_CORPUS, else tools/corpus/pcb-spectra.
#
# The oracles run from a venv under the target dir:
#   python3 -m venv target/pcb-spectra/venv
#   target/pcb-spectra/venv/bin/pip install --no-cache-dir numpy scipy gerbonara pygerber \
#     kiutils jcamp pyteomics pynumpress lxml nmrglue setuptools wheel
#   target/pcb-spectra/venv/bin/pip install --no-cache-dir --no-build-isolation spc-spectra
#
# Downloads are untrusted data: this script only copies and unpacks files, never runs anything
# from them. Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_PCB_SPECTRA_CORPUS:-$ROOT/tools/corpus/pcb-spectra}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PARTS=("$@")
if (( ${#PARTS[@]} == 0 )); then
  PARTS=(gerber kicad jcamp mzml nmr)
fi

die() { echo "fetch-pcb-spectra: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  mkdir -p "$OUT"
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL SHA DEST: downloads URL to DEST unless DEST already has that SHA-256.
get() {
  local url=$1 sha=$2 dest=$3
  if [[ -s "$dest" ]] && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return 0
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 -o "$dest.part" "$url" || die "download failed: $url"
  local got
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "SHA-256 mismatch for $url: got $got, want $sha"
  fi
  mv "$dest.part" "$dest"
}

for part in "${PARTS[@]}"; do
  list="$ROOT/tools/corpus/pcb-spectra-$part.tsv"
  [[ -f $list ]] || die "no list for part '$part' ($list)"
  n=0
  while IFS=$'\t' read -r url sha dest lic; do
    [[ -z ${url// } || $url == \#* ]] && continue
    base="$OUT/$part"
    if [[ $dest == */ ]]; then
      # An archive unpacked into a folder; the archive itself is kept beside it.
      arc="$base/.archives/$(basename "$url")"
      get "$url" "$sha" "$arc"
      if [[ ! -e "$base/$dest.unpacked" ]]; then
        mkdir -p "$base/$dest"
        case $url in
          *.zip) unzip -q -o "$arc" -d "$base/$dest" ;;
          *.tar.gz|*.tgz) tar -xzf "$arc" -C "$base/$dest" --no-same-owner --no-same-permissions ;;
          *.tar.bz2) tar -xjf "$arc" -C "$base/$dest" --no-same-owner --no-same-permissions ;;
          *) die "cannot unpack $url" ;;
        esac
        touch "$base/$dest.unpacked"
      fi
    else
      get "$url" "$sha" "$base/$dest"
    fi
    printf '%s\t%s\t%s\n' "$part/$dest" "$url" "${lic:-unknown}" >> "$OUT/.provenance.tsv.new"
    n=$((n + 1))
  done < "$list"
  echo "fetch-pcb-spectra: $part: $n entries"
done
if [[ -f "$OUT/.provenance.tsv.new" ]]; then
  sort -u "$OUT/.provenance.tsv.new" > "$OUT/.provenance.tsv"
  rm -f "$OUT/.provenance.tsv.new"
fi
du -sh "$OUT" | cut -f1 | sed 's/^/fetch-pcb-spectra: corpus size /'
