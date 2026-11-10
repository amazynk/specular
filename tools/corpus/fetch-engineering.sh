#!/usr/bin/env bash
# Fetches the engineering-formats test corpus (IPC-2581, ODB++, LandXML samples) for oracle runs.
# Nothing here is committed: files go to $SPECULAR_ENGINEERING_CORPUS, else
# ${CARGO_TARGET_DIR:-target}/corpus/engineering.
#
# Usage: tools/corpus/fetch-engineering.sh [PART...]
#   PART is ipc2581, odb or landxml (default: all). Files are listed in tools/corpus/engineering.tsv:
#   part, URL, SHA-256, destination (relative to OUT/PART), licence; tab-separated, '#' starts
#   a comment. Each file is checked against its SHA-256 (a mismatch removes it and stops).
#   A destination ending in / takes a .zip and unpacks it there.
#
# Downloads are untrusted data: this script only copies and unpacks, never runs anything from
# them. It stops when free space drops below MIN_FREE_GB (default 8). Every download shows
# what it fetches with a progress bar (speed and time left).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_ENGINEERING_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/engineering}
MIN_FREE_GB=${MIN_FREE_GB:-8}
LIST=$ROOT/tools/corpus/engineering.tsv
PARTS=("$@")
if (( ${#PARTS[@]} == 0 )); then
  PARTS=(ipc2581 odb landxml)
fi

die() { echo "fetch-engineering: $*" >&2; exit 1; }

check_space() {
  mkdir -p "$OUT"
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

get() {
  local url=$1 sha=$2 dest=$3
  if [[ -s "$dest" ]] && [[ $(sha256sum "$dest" | cut -d' ' -f1) == "$sha" ]]; then
    return 0
  fi
  check_space
  mkdir -p "$(dirname "$dest")"
  echo "Downloading $(basename "$dest") from $url"
  curl -fL --retry 3 --progress-bar -o "$dest.part" "$url" || die "download failed: $url"
  local got
  got=$(sha256sum "$dest.part" | cut -d' ' -f1)
  if [[ $sha != "-" && $got != "$sha" ]]; then
    rm -f "$dest.part"
    die "SHA-256 mismatch for $url: got $got, want $sha"
  fi
  [[ $sha == "-" ]] && echo "  sha256 $got (not pinned yet)"
  mv "$dest.part" "$dest"
}

while IFS=$'\t' read -r part url sha dest licence; do
  [[ -z ${part:-} || $part == \#* ]] && continue
  want=0
  for p in "${PARTS[@]}"; do [[ $p == "$part" ]] && want=1; done
  (( want )) || continue
  if [[ $dest == */ ]]; then
    zip="$OUT/$part/.dl/$(basename "$url")"
    get "$url" "$sha" "$zip"
    mkdir -p "$OUT/$part/$dest"
    unzip -qo "$zip" -d "$OUT/$part/$dest"
  else
    get "$url" "$sha" "$OUT/$part/$dest"
  fi
  echo "  $part/$dest ($licence)"
done < "$LIST"
echo "Corpus in $OUT"
