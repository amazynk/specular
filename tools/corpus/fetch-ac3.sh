#!/usr/bin/env bash
# Fetch real-world AC-3 / E-AC-3 streams for specular-ac3 (gitignored: nothing here is
# committed) and write the oracle decodes beside each stream.
#
# Usage: tools/corpus/fetch-ac3.sh
#   OUT defaults to $SPECULAR_AC3_CORPUS, else target/ac3-corpus.
#   Then: SPECULAR_AC3_CORPUS=<OUT> cargo test -p specular-ac3 --test corpus -- --nocapture
#
# What it covers (about 6 MB), from FFmpeg's FATE suite (fate-suite.ffmpeg.org/ac3 and /eac3,
# commercial encoder output), SHA-256 pinned:
#   AC-3    monsters_inc_2.0_192_small (2/0), monsters_inc_5.1_448_small (3/2 + LFE),
#           millers_crossing_4.0 (2/2)
#   E-AC-3  csi_miami_stereo_128_spx_small, csi_miami_5.1_256_spx_small (spectral
#           extension), matrix2_commentary1_stereo_192_small, serenity_english_5.1_1536_small,
#           the_great_wall_7.1 (a 5.1 programme with a dependent substream)
# If ffmpeg is on PATH, its float decode is written as <name>.f32 (every channel) and
# <name>.st.f32 (its stereo downmix), interleaved f32le.
#
# Downloads are untrusted data: this script only copies files and hands them to ffmpeg.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_AC3_CORPUS:-$ROOT/target/ac3-corpus}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-ac3: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get DIR NAME SHA256: an empty hash prints the hash instead of checking it (for pinning).
get() {
  local dir=$1 name=$2 sum=$3 base=https://fate-suite.ffmpeg.org/$1
  check_space
  if [[ ! -s "$OUT/$name" ]]; then
    curl -fsSL --retry 3 -o "$OUT/$name.part" "$base/$name" || { rm -f "$OUT/$name.part"; die "download failed: $base/$name"; }
    mv "$OUT/$name.part" "$OUT/$name"
  fi
  local got
  got=$(sha256sum "$OUT/$name" | cut -d' ' -f1)
  if [[ -z $sum ]]; then
    echo "unpinned $name $got"
  elif [[ $got != "$sum" ]]; then
    rm -f "$OUT/$name"
    die "$name: SHA-256 $got, expected $sum"
  fi
  printf '%s\t%s\t%s\n' "$name" "$base/$name" "$got" >> "$OUT/.provenance.tsv"
  if command -v ffmpeg > /dev/null; then
    local stem=${name%.*}
    ffmpeg -nostdin -loglevel error -y -i "$OUT/$name" -f f32le "$OUT/$stem.f32"
    ffmpeg -nostdin -loglevel error -y -downmix stereo -i "$OUT/$name" -ac 2 -f f32le "$OUT/$stem.st.f32"
  fi
}

get ac3 monsters_inc_2.0_192_small.ac3 4df071457c23d1a39fe52322892bc9333921ad6dfc8f87ffa0797bdfb5101bae
get ac3 monsters_inc_5.1_448_small.ac3 74fefeef070f663378a0864beb064cf88ee1186152f7ac320984af15be2f22db
get ac3 millers_crossing_4.0.ac3 faa06c7b1ccbc5cf3c7897afb50004dd3d49feaa19d26bdeb03dc9bb4ec20802
get eac3 csi_miami_stereo_128_spx_small.eac3 adc397f12472353237f7dc8139c2223937abc23eff9cd80ac2e3471814a04c3a
get eac3 csi_miami_5.1_256_spx_small.eac3 84f50317a49a98509bf174ace814b39860e6ab2a1d61bdbd10ada9081f2bb2b3
get eac3 matrix2_commentary1_stereo_192_small.eac3 ea095d557bd6787266f4b7276a2be818a7fbc924997748be89d9ead962a3391c
get eac3 serenity_english_5.1_1536_small.eac3 3d81c22610a2602b7799ec5e59c2677e8ab23e03e4abc03c2a16b683aa3d5f0a
get eac3 the_great_wall_7.1.eac3 d698981ddc1c10e65d234dd6a02ff2b0f4c88787595c9d3e5adf7f097f2c9e50
echo "fetch-ac3: corpus in $OUT"
