#!/usr/bin/env bash
# Fetch PE Core S16 (Meta's Perception Encoder) for the specular-infer reference tests (never
# committed).
#
# Usage: tools/corpus/fetch-pecore.sh [model|tokenizer|oracle|kodak...]   (default: all four)
#   OUT defaults to $SPECULAR_PECORE_DIR, else target/pecore.
#
#   model      PE-Core-S16-384.pt (facebook/PE-Core-S16-384, Apache-2.0), 349 MB: what the
#              pack downloads
#   tokenizer  merges.txt (timm/PE-Core-S-16-384, Apache-2.0): CLIP's BPE merges, the same
#              48,894 the reference ships as bpe_simple_vocab_16e6.txt.gz
#   oracle     open_clip_model.safetensors (timm/PE-Core-S-16-384, Apache-2.0), 349 MB: the
#              same weights converted by timm; the Python oracle (tools/corpus/pecore-oracle.py)
#              runs them through open_clip and timm's own PE implementation
#   kodak      the 24 Kodak test photos (released for unrestricted use)
#
# Files come at pinned commits and are checked against their sha256. Downloads are untrusted
# data: this script only copies and hashes them. Resumes a partial download (curl -C -).
# Stops when free space drops below MIN_FREE_GB (default 8).
#
# The tests (crates/specular-infer/tests/pecore.rs) read SPECULAR_PECORE_DIR and skip with a
# reason when the files are absent.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_PECORE_DIR:-$ROOT/target/pecore}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-pecore: $*" >&2; exit 1; }

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

PE=https://huggingface.co/facebook/PE-Core-S16-384/resolve/aabf3b990573d8114ae6e501b4697106beac8f19
TIMM=https://huggingface.co/timm/PE-Core-S-16-384/resolve/3249a38eb1c432ec19231c6fe1774acb6a4e4efe

model() { get "$PE/PE-Core-S16-384.pt" PE-Core-S16-384.pt ccc8340a14ea3ebf557a288ba4ed4a5bc026ab98bb4da42fc745d44b4c5c5ffb; }
tokenizer() { get "$TIMM/merges.txt" merges.txt 9fd691f7c8039210e0fced15865466c65820d09b63988b0174bfe25de299051a; }
oracle() { get "$TIMM/open_clip_model.safetensors" open_clip_model.safetensors 440da04ea6f08f5ce07b2e7118735660c7a71f981bdf3edde8e8b15f28d660b0; }

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
[[ ${#sections[@]} -eq 0 ]] && sections=(model tokenizer oracle kodak)
for s in "${sections[@]}"; do
  case $s in
    model|tokenizer|oracle|kodak) echo "$s:"; "$s" ;;
    *) die "unknown section $s" ;;
  esac
done
