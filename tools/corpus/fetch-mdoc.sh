#!/usr/bin/env bash
# Fetch public SerialEM .mdoc examples into tools/corpus/mdoc/ (gitignored: nothing here is
# committed) and write their oracle. The specular-em test `mdoc` reads them in place from
# SPECULAR_MDOC_CORPUS (a worktree has no corpus of its own), else tools/corpus/mdoc, and skips
# when they are absent.
#
# Usage: tools/corpus/fetch-mdoc.sh
#   OUT defaults to $SPECULAR_MDOC_CORPUS, else tools/corpus/mdoc.
#   PYTHON (default python3) must have mdocfile for the oracle; a venv under target/ is fine:
#   python3 -m venv target/mdoc-venv && target/mdoc-venv/bin/pip install --no-cache-dir mdocfile
#
# What it covers (about 140 KB): mdocfile's test data (teamtomo/mdocfile, BSD-3-Clause): a tilt
# series, frame sets and montages. expected-mdoc.tsv is the oracle (mdocfile).
#
# Downloads are untrusted data: every file is checked against its SHA-256 below, and nothing
# from them is run (the oracle reads them with python -I from this repository's script).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MDOC_CORPUS:-$ROOT/tools/corpus/mdoc}
PYTHON=${PYTHON:-python3}
BASE=https://raw.githubusercontent.com/teamtomo/mdocfile/de4e3c7a1f373a46ba5cee0cdc47b671b329cb34/tests/test_data
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

while read -r want name; do
  out="$OUT/$name"
  if [[ ! -s $out ]]; then
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" "$BASE/$name"
    mv "$out.part" "$out"
  fi
  echo "$want  $out" | sha256sum -c --quiet - || { echo "fetch-mdoc: checksum mismatch: $name" >&2; exit 1; }
  printf '%s\t%s\t%s\n' "$name" "$BASE/$name" "BSD-3-Clause" >> "$OUT/.provenance.tsv"
done <<'LIST'
d5dcba7bb90e096392f126259638f4eeacc1acff2c1a05077611c0bb93e6b749 frame_set_multiple.mdoc
aaf5c5642ce8d19e66323009781271ef205a4becdda3253d57910ac67f76bb50 frame_set_single.mdoc
8826d1aaa9fe780f5999fb75eebeb4cadaf2e8803d609b4c622547a9d998ebb1 montage_section.mdoc
28a38ea31e2339df48adb6b968c200d47d08c0e140734f938bc3b691c03959b6 montage_section_multiple.mdoc
fd53ede29820450c61f48fed91aec181b703afc687405f87ffc05f406f505652 tilt_series.mdoc
LIST

"$PYTHON" -I "$ROOT/crates/formats/specular-em/gen/make_mdoc.py" "$OUT"
echo "fetch-mdoc: done ($OUT)"
