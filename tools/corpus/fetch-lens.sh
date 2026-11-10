#!/usr/bin/env bash
# Fetch the lens-correction corpus: Sony ARW and Fujifilm RAF files whose raw data carries
# the camera's lens correction tables (distortion, vignetting, colour fringes), for Develop's
# lens models (crates/specular-develop/tests/lens_oracle.rs, tools/quality/lens_oracle.py).
# All from raw.pixls.us (CC0), pinned by path and SHA-256. About 165 MB.
#
#   tools/corpus/fetch-lens.sh [DEST]
#
# DEST defaults to $LENS_CORPUS_DIR or tools/corpus/lens (gitignored).
#
# Downloads are untrusted data: this script only copies files, never runs anything from them.
# Stops if free space on the destination drops below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DEST=${1:-${LENS_CORPUS_DIR:-$ROOT/tools/corpus/lens}}
MIN_FREE_GB=${MIN_FREE_GB:-6}
mkdir -p "$DEST"

die() { echo "fetch-lens: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$DEST" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# SHA-256 | path under https://raw.pixls.us/data/   # lens, what the camera corrected
FILES=(
  # E PZ 18-105mm F4 G at 105 mm: pincushion, distortion corrected in camera
  '8a37c80fc47c6026efa2e55279220e79460e2f470b1aa48b440f8f2d47873aa6|Sony/ILCE-6400/DSC00087.ARW'  # 25 MB
  # FE 16-35mm F2.8 GM at 32 mm: distortion correction off in camera (the JPEG is not corrected)
  '250784580ea527442c09004417bb0eead484f2bf3ee8f9121a776ac65bb50d0f|Sony/ILCE-7M3/_DSC0009.ARW'  # 26 MB
  # XF35mmF2 R WR, XF16-55mmF2.8 at 16 mm: barrel, colour fringes, vignetting
  '24abd27b4a200f3170ea1579672944b807d0eb35bf21422a3937bb644f27f2eb|Fujifilm/X-T3/AFXT2720.RAF'  # 56 MB
  '76f70724b58014fc1c45f6d6f5d0e0781bf08e9537b72de866446bda9a24bb0c|Fujifilm/X-T4/DSCF0266.RAF'  # 58 MB
)

for entry in "${FILES[@]}"; do
  sha=${entry%%|*}
  rel=${entry#*|}
  out="$DEST/$rel"
  if [[ ! -s $out ]] || ! echo "$sha  $out" | sha256sum -c --quiet - 2>/dev/null; then
    check_space
    mkdir -p "$(dirname "$out")"
    enc=$(python3 -I -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$rel")
    curl -fsSL --retry 3 -A 'specular-corpus-fetch/1 (test corpus)' -o "$out.part" \
      "https://raw.pixls.us/data/$enc"
    mv "$out.part" "$out"
    echo "$sha  $out" | sha256sum -c --quiet - || die "checksum mismatch: $rel"
  fi
done
echo "fetch-lens: ${#FILES[@]} files in $DEST"
