#!/usr/bin/env bash
# Fetch the ISO/IEC 11172-4 Layer III compliance streams for specular-mpa (gitignored:
# nothing here is committed) and write ffmpeg's decode beside each stream.
#
# Usage: tools/corpus/fetch-mp3.sh
#   OUT defaults to $SPECULAR_MP3_CORPUS, else $CARGO_TARGET_DIR/corpus/mp3, else
#   target/corpus/mp3.
#   Then: SPECULAR_MP3_CORPUS=<OUT> cargo test -p specular-mpa --test corpus -- --ignored
#
# What it covers (about 3 MB), from FFmpeg's FATE suite (fate-suite.ffmpeg.org/
# mp3-conformance, the ISO compliance bit streams with their reference decodes), SHA-256
# pinned: compl (the full compliance stream), he_32khz/he_44khz/he_48khz (all rates),
# he_mode (mode changes: joint stereo with intensity and middle/side), hecommon, si,
# si_block (block switching, mixed blocks), si_huff (every Huffman table), sin1k0db, and
# he_free (free format, which the decoder refuses).
# If ffmpeg is on PATH, its float decode is written as <name>.f32 (stereo, interleaved
# f32le; mono frames at -3 dB on each side, as ffmpeg spreads them).
#
# Downloads are untrusted data: this script only copies files and hands them to ffmpeg.
# Stops if free space on the target filesystem drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_MP3_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/mp3}
MIN_FREE_GB=${MIN_FREE_GB:-8}
mkdir -p "$OUT"
: > "$OUT/.provenance.tsv"

die() { echo "fetch-mp3: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$OUT" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

# get NAME SHA256: an empty hash prints the hash instead of checking it (for pinning).
get() {
  local name=$1 sum=$2 base=https://fate-suite.ffmpeg.org/mp3-conformance
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
  if [[ $name == *.bit ]] && command -v ffmpeg > /dev/null; then
    ffmpeg -nostdin -loglevel error -y -f mp3 -i "$OUT/$name" -ac 2 -f f32le "$OUT/${name%.bit}.f32" || true
  fi
}

get compl.bit 3b93f3675821a5af4826abcff7097fa2e42efcc46154e02c992995f7e31dbc8e
get he_32khz.bit 965065cdcac3f45a61a9f54b4f94d59d0334f4b5cb74840a5aec7d405e2e050f
get he_44khz.bit a64212281a8896d90e00f06c9b21f2c8f21ef9cb7367cae692a92ea421984441
get he_48khz.bit dd0383075710570648b8961f3cd0ce692f07a679b265fc4997e50883b3958299
get he_mode.bit fb90396d9bb13311be35284d061e19196830d213d823e05297b18172c88d90ce
get hecommon.bit 21999b77716aeba047c7f55c2c1ce17b54c780b6dc007912f251b7ea189676ef
get si.bit 4f3382de1d9265013d42249ff108023d4a90ecf3c0136489d199243a90a5d0f9
get si_block.bit 1d12956840638bf6b81164fc510c3e8269eeb038a51dc4548469e21a71e52b69
get si_huff.bit fda6d8a7d4e1df3cb17d3560b57013ef98abbf9c60d940455c2e3ebf63eae0ef
get sin1k0db.bit 64775e23c29aa84e27f68777460f88e0717af2ed64e732d287a20c5a92c9fa3e
get he_free.bit b55afc2a21492c4b3035d423f36fe87560ef34f65eb997da60af9b01f300362c
# The Huffman tables' cross-check: minimp3.h (CC0 1.0) at a pinned commit, whose packed
# tables encode ISO/IEC 11172-3 Annex B; mp3-tables-check.py compares every code word.
MM_COMMIT=ea99364f61c14656440e8d77e9c233ccf3124633
MM_URL=https://raw.githubusercontent.com/lieff/minimp3/$MM_COMMIT/minimp3.h
mkdir -p "$OUT/ref"
if [[ ! -s "$OUT/ref/minimp3.h" ]]; then
  check_space
  curl -fsSL --retry 3 -o "$OUT/ref/minimp3.h.part" "$MM_URL" || die "download failed: $MM_URL"
  mv "$OUT/ref/minimp3.h.part" "$OUT/ref/minimp3.h"
fi
got=$(sha256sum "$OUT/ref/minimp3.h" | cut -d' ' -f1)
[[ $got == 57e437c5c1f0e8b243885d3929c8973b5e6c778451e0100ab4251d19915cb3ad ]] || { rm -f "$OUT/ref/minimp3.h"; die "minimp3.h: SHA-256 $got"; }
printf '%s\t%s\t%s\n' ref/minimp3.h "$MM_URL" "$got" >> "$OUT/.provenance.tsv"
python3 -I "$ROOT/tools/corpus/mp3-tables-check.py" "$OUT/ref/minimp3.h" "$ROOT/crates/formats/specular-mpa/src/huffman_data.rs"
echo "fetch-mp3: corpus in $OUT"
