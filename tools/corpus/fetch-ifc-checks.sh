#!/usr/bin/env bash
# Fetch the real building models the IFC check tests use (model information read against
# IfcOpenShell, clash, diff and quantity timings) into tools/corpus/ifc-checks/ (gitignored).
# The specular-bim test `ifc_checks_corpus` reads them from SPECULAR_IFC_CHECKS_CORPUS, else
# tools/corpus/ifc-checks, and skips when they are absent.
#
# Usage: tools/corpus/fetch-ifc-checks.sh [--oracle PYTHON]
#   --oracle runs tools/corpus/ifc-data-oracle.py with PYTHON (it needs ifcopenshell; a venv
#   under target/ is fine, e.g. target/ifc2/venv/bin/python) and writes the
#   `*.oracle.json` files next to the models.
#
# Source: web-ifc's public test models (repository MPL-2.0; the models from their public
# sources; dev use), pinned commit, about 120 MB. Every file is checked against its SHA-256.
# Downloads are untrusted data: this script only copies them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_IFC_CHECKS_CORPUS:-$ROOT/tools/corpus/ifc-checks}
BASE=https://raw.githubusercontent.com/ThatOpen/engine_web-ifc/dd44aa77e770d2584b6d4095dec0d89f4a4cf6d0/tests/ifcfiles/public
mkdir -p "$OUT"

die() { echo "fetch-ifc-checks: $*" >&2; exit 1; }

while read -r sha name; do
  [[ -z "$sha" ]] && continue
  dest="$OUT/$name"
  if [[ -f "$dest" ]] && echo "$sha  $dest" | sha256sum -c --status; then
    continue
  fi
  echo "fetch-ifc-checks: $name"
  curl -fsSL --retry 3 -o "$dest.part" "$BASE/$name" || die "download failed: $name"
  echo "$sha  $dest.part" | sha256sum -c --status || { rm -f "$dest.part"; die "SHA-256 mismatch: $name"; }
  mv "$dest.part" "$dest"
done <<'LIST'
70cc8ff245fc0894201d96496c031005a5cbd7a96b22d8a1b87c5a883fb77994 AC20-FZK-Haus.ifc
b347a2c8aa8fff6db896a4417a9c50c22ac0ccd7c5cfc22b99b8d29336c606ed duplex.ifc
3ccc7df480d7ae9d1af148d13634fe1af8f1a75949fdf3cb1f87c60464df4700 Office_A_20110811.ifc
71c9466bf02306be58d45b77d738541a0e7410767960b5eed1cf85737dfa34c9 IfcOpenHouse_IFC4.ifc
2ac970ce065ecac4e0c9e5f453a257169e90d0067f419b7e33533a64ef837880 dental_clinic.ifc
2c3565ca1904f2aa61adab92024cf3755b2c5b21a498144d3094d7cb58cebec7 schependomlaan.ifc
369b482dee0a277354e64dec7ea15b957bbb21a0f4cbf05478050f62fc0dde12 ISSUE_053_20181220Holter_Tower_10.ifczip
LIST

if [[ "${1:-}" == "--oracle" ]]; then
  PY=${2:?--oracle needs a python}
  for f in "$OUT"/*.ifc; do
    "$PY" -I "$ROOT/tools/corpus/ifc-data-oracle.py" "$f" "${f%.ifc}.oracle.json"
  done
fi
echo "fetch-ifc-checks: done ($OUT)"
