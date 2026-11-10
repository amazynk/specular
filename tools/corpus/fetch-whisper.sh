#!/usr/bin/env bash
# Fetch the Whisper checkpoints the Local AI pack converts to f16 on install (base, small),
# so `crates/specular-ai/tests/pins.rs` can check the converted-file pins (never committed).
#
# Usage: tools/corpus/fetch-whisper.sh [base|small...]   (default: both)
#   OUT defaults to $SPECULAR_WHISPER_DIR, else target/originals/whisper.
#
#   base   openai/whisper-base model.safetensors (MIT), 290 MB
#   small  openai/whisper-small model.safetensors (MIT), 967 MB
#
# The same pinned commits and sha256 as crates/specular-ai/manifest/models.toml. Downloads
# are untrusted data: this script only copies and hashes them. Resumes a partial download
# (curl -C -). Stops when free space drops below MIN_FREE_GB (default 8). Then:
#   SPECULAR_AI_ORIGINALS=target/originals cargo test -p specular-ai --test pins -- --nocapture
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_WHISPER_DIR:-$ROOT/target/originals/whisper}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"

die() { echo "fetch-whisper: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get URL DEST SHA256: downloads one file (resuming, with curl's progress bar) and checks it.
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
  printf '%s\t%s\tMIT\n' "$dest" "$url" >> "$OUT/.provenance.tsv"
}

HF=https://huggingface.co/openai

base() { get "$HF/whisper-base/resolve/e37978b90ca9030d5170a5c07aadb050351a65bb/model.safetensors" base/model.safetensors 07cadb9f25677c8d50df603e66a98fbd842cce45047139baeb16e6219a1e807b; }
small() { get "$HF/whisper-small/resolve/973afd24965f72e36ca33b3055d56a652f456b4d/model.safetensors" small/model.safetensors 1d7734884874f1a1513ed9aa760a4f8e97aaa02fd6d93a3a85d27b2ae9ca596b; }

sections=("$@")
[[ ${#sections[@]} -eq 0 ]] && sections=(base small)
for s in "${sections[@]}"; do
  case $s in
    base|small) echo "$s:"; "$s" ;;
    *) die "unknown section $s" ;;
  esac
done
