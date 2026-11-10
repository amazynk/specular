#!/usr/bin/env bash
# Fetch the Clip Studio Paint sample files into tools/corpus/design2/clip/ (gitignored). The
# specular-clip corpus test reads SPECULAR_CLIP_CORPUS, else that folder, and skips when it is
# absent; it checks our SQLite reader against SQLite itself (preview blob, layer names).
#
# Sources (test files of MIT-licensed open-source projects, pinned by sha256):
#   dobrokot/clip_to_psd tests/test_export_all_features.clip (3.2 MB, 928×647, 117 layers)
#   lycoris0731/clip tests/assets/sample.clip (10.6 MB, 500×310, 89 layers)
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_CLIP_CORPUS:-$ROOT/tools/corpus/design2/clip}
mkdir -p "$OUT"

get() {
  local name=$1 url=$2 sha=$3
  if [[ -f "$OUT/$name" ]] && echo "$sha  $OUT/$name" | sha256sum -c --quiet 2>/dev/null; then
    echo "fetch-design2-clip: $name present"
    return
  fi
  echo "fetch-design2-clip: $name <- $url"
  curl -fsSL --retry 3 -o "$OUT/$name.part" "$url"
  echo "$sha  $OUT/$name.part" | sha256sum -c --quiet || { rm -f "$OUT/$name.part"; echo "checksum mismatch: $name" >&2; exit 1; }
  mv "$OUT/$name.part" "$OUT/$name"
}

get clip_to_psd-all-features.clip \
  https://raw.githubusercontent.com/dobrokot/clip_to_psd/HEAD/tests/test_export_all_features.clip \
  5e342dd8b397ed6c915b701bf77151cccafd93cc684bbe09439b91d8102c4e8e
get lycoris-sample.clip \
  https://raw.githubusercontent.com/lycoris0731/clip/HEAD/tests/assets/sample.clip \
  a313134c226fe80a756a36ba87657b028321b7d2f23971bb2ede86d847f784bf
