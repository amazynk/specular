#!/usr/bin/env bash
# Fetch the buildingSMART IDS 1.0 implementers' test cases into tools/corpus/ids/ (gitignored:
# nothing downloaded here is committed). Each case is an .ids and an .ifc of the same name,
# `pass-*` or `fail-*`: checking the model against the IDS must pass or fail. The
# specular-bim test `ids_test_cases` reads them from SPECULAR_IDS_CORPUS, else
# tools/corpus/ids, and skips when they are absent.
#
# Usage: tools/corpus/fetch-ids.sh
#
# Source: https://github.com/buildingSMART/IDS, Documentation/ImplementersDocumentation/
# TestCases at the pinned commit below (334 pairs, about 700 KB). Licence: (c) buildingSMART
# International Ltd., CC BY-ND 4.0; kept out of the repository. Every file is checked against
# its SHA-256 in tools/corpus/ids.sha256; a mismatch stops the script. Downloads are
# untrusted data: this script only copies them.
#
# Second oracle (optional): IfcOpenShell's ifctester in a venv under target/, e.g.
#   python3 -m venv target/ifc2/venv
#   PIP_NO_CACHE_DIR=1 target/ifc2/venv/bin/pip install ifctester
#   target/ifc2/venv/bin/python -I tools/corpus/ids-oracle.py tools/corpus/ids
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_IDS_CORPUS:-$ROOT/tools/corpus/ids}
LIST="$ROOT/tools/corpus/ids.sha256"
COMMIT=870f9c4e6e8f414e737b4d84ca1aa9b46fc6c8f3
BASE="https://raw.githubusercontent.com/buildingSMART/IDS/$COMMIT/Documentation/ImplementersDocumentation/TestCases"
mkdir -p "$OUT"

die() { echo "fetch-ids: $*" >&2; exit 1; }

n=0
while read -r sha rel; do
  [[ -z "$sha" ]] && continue
  dest="$OUT/$rel"
  if [[ -f "$dest" ]] && echo "$sha  $dest" | sha256sum -c --status; then
    n=$((n + 1))
    continue
  fi
  mkdir -p "$(dirname "$dest")"
  curl -fsSL --retry 3 -o "$dest.part" "$BASE/$rel" || die "download failed: $rel"
  echo "$sha  $dest.part" | sha256sum -c --status || { rm -f "$dest.part"; die "SHA-256 mismatch: $rel"; }
  mv "$dest.part" "$dest"
  n=$((n + 1))
done < "$LIST"
echo "fetch-ids: $n files in $OUT"
