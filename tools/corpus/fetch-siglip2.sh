#!/usr/bin/env bash
# Fetch the SigLIP 2 search models for the specular-infer reference tests (never committed).
#
# Usage: tools/corpus/fetch-siglip2.sh [b16|b32|tokenizer...]   (default: tokenizer b16 kodak)
#   OUT defaults to $SPECULAR_SIGLIP2_DIR, else target/siglip2. Each model goes in its own
#   folder (OUT/siglip2-b16-256, OUT/siglip2-b32-256) with tokenizer.model next to it.
#
# Files come from Hugging Face at pinned commits and are checked against the LFS sha256
# (Apache-2.0: https://huggingface.co/google/siglip2-base-patch16-256). Downloads are
# untrusted data: this script only copies and hashes them. Resumes a partial download
# (curl -C -). Stops when free space drops below MIN_FREE_GB (default 8).
#
# The tests (crates/specular-infer/tests/siglip.rs) read SPECULAR_SIGLIP2_DIR and skip with a
# reason when the files are absent. tokenizer.json is fetched too: only the Python oracle
# (tools/corpus/siglip2-oracle.py) reads it.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_SIGLIP2_DIR:-$ROOT/target/siglip2}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-siglip2: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST SHA256: downloads one file (resuming) and checks its hash.
get() {
  local url=$1 dest=$2 sha=$3
  check_space
  mkdir -p "$(dirname "$OUT/$dest")"
  if [[ -f "$OUT/$dest" ]] && echo "$sha  $OUT/$dest" | sha256sum -c --quiet - 2>/dev/null; then
    echo "  $dest (cached)"
    return
  fi
  echo "  $dest"
  curl -fL --retry 3 -C - -o "$OUT/$dest.part" "$url"
  echo "$sha  $OUT/$dest.part" | sha256sum -c --quiet - || die "$dest: hash mismatch"
  mv "$OUT/$dest.part" "$OUT/$dest"
  printf '%s\t%s\tApache-2.0\n' "$dest" "$url" >> "$OUT/.provenance.tsv"
}

# get_plain URL DEST: a small git (non-LFS) file at a pinned commit.
get_plain() {
  [[ -f "$OUT/$2" ]] && return
  curl -fsL -o "$OUT/$2" "$1"
}

B16=https://huggingface.co/google/siglip2-base-patch16-256/resolve/3f9f96cb90da5dbc758b01813f2f6f1aee24c1ab
B32=https://huggingface.co/google/siglip2-base-patch32-256/resolve/94dffa8cb1179de3e03f091dbc3917e5d5a9ae84
TOK_SHA=61a7b147390c64585d6c3543dd6fc636906c9af3865a5548f27f31aee1d4c8e2

tokenizer() {
  get "$B16/tokenizer.model" tokenizer.model $TOK_SHA
  get "$B16/tokenizer.json" tokenizer.json cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322
  for f in tokenizer_config.json special_tokens_map.json; do get_plain "$B16/$f" "$f"; done
}

model() {
  local dir=$1 base=$2 sha=$3
  get "$base/model.safetensors" "$dir/model.safetensors" "$sha"
  for f in config.json preprocessor_config.json; do get_plain "$base/$f" "$dir/$f"; done
}

b16() { model siglip2-b16-256 "$B16" 6125cacc01fa93bdc98a0c5101cefcd69b2ed1f8ab4f38d86f4ad5984f5dc863; }
b32() { model siglip2-b32-256 "$B32" 7d241bb3becad218f211f480487f491df4f8c0a472ecf7afdec5615815a301f1; }

# The Kodak test photos (24 PNGs, 768x512; Kodak, released for unrestricted use): the
# images the embeddings and the multilingual check set are measured on.
kodak() {
  local i
  mkdir -p "$OUT/kodak"
  for i in $(seq -w 1 24); do
    [[ -s "$OUT/kodak/kodim$i.png" ]] && continue
    check_space
    curl -fsSL -o "$OUT/kodak/kodim$i.png" "https://r0k.us/graphics/kodak/kodak/kodim$i.png"
    printf '%s\t%s\t%s\n' "kodak/kodim$i.png" "https://r0k.us/graphics/kodak/" \
      'Kodak, released for unrestricted use' >> "$OUT/.provenance.tsv"
  done
}

sections=("$@")
[[ ${#sections[@]} -eq 0 ]] && sections=(tokenizer b16 kodak)
for s in "${sections[@]}"; do
  case $s in
    tokenizer|b16|b32|kodak) echo "$s:"; "$s" ;;
    *) die "unknown section $s" ;;
  esac
done
