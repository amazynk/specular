#!/usr/bin/env bash
# Run every cargo-fuzz target of one fuzz crate for a fixed time each.
#
#   tools/fuzz/run.sh <crate dir> [seconds per target] [extra libFuzzer args...]
#   tools/fuzz/run.sh crates/formats/specular-png 60
#
# <crate dir> is the decoder crate (the one holding `fuzz/`). Before running, each target's
# corpus (`fuzz/corpus/<target>`, gitignored) is seeded from:
#   - `fuzz/seeds/*`          small synthetic files, committed, given to every target;
#   - `fuzz/seeds/<target>/*` the same, for one target;
#   - `fuzz/seed-fixtures`    globs (relative to the crate dir, one per line) of existing
#                             test fixtures; only files up to 64 KiB are used.
# `fuzz/fuzz.dict` (or `fuzz/dict/<target>.dict`), when present, is passed as a libFuzzer
# dictionary. Crashes land in `fuzz/artifacts/<target>/`.
#
# Environment: FUZZ_RSS_MB (default 2048) caps the fuzzer's memory; FUZZ_BUILD_FLAGS is passed
# to `cargo fuzz run` (e.g. `-a` for debug assertions and overflow checks).
# FUZZ_TARGETS (space-separated) runs only those targets.
set -euo pipefail

crate=${1:?usage: run.sh <crate dir> [seconds] [libFuzzer args...]}
secs=${2:-60}
shift $(($# >= 2 ? 2 : 1))
rss=${FUZZ_RSS_MB:-2048}
# Decoders use rayon; keep a fuzzer from taking every core.
export RAYON_NUM_THREADS=${RAYON_NUM_THREADS:-2}
# System libraries' process-lifetime allocations are not leaks (tools/fuzz/lsan.supp).
supp="$(cd "$(dirname "$0")" && pwd)/lsan.supp"
export LSAN_OPTIONS="suppressions=$supp${LSAN_OPTIONS:+:$LSAN_OPTIONS}"

cd "$crate"
here=$PWD
[ -d fuzz ] || { echo "$crate has no fuzz/ crate" >&2; exit 1; }

targets=${FUZZ_TARGETS:-$(cargo fuzz list)}
[ -n "$targets" ] || { echo "no fuzz targets found in $crate" >&2; exit 1; }

for t in $targets; do
  corpus=fuzz/corpus/$t
  mkdir -p "$corpus" "fuzz/artifacts/$t"
  find fuzz/seeds -maxdepth 1 -type f -exec cp -n {} "$corpus/" \; 2>/dev/null || true
  [ -d "fuzz/seeds/$t" ] && find "fuzz/seeds/$t" -type f -exec cp -n {} "$corpus/" \;
  if [ -f fuzz/seed-fixtures ]; then
    while read -r glob; do
      case $glob in '' | '#'*) continue ;; esac
      # shellcheck disable=SC2086 # the glob is meant to expand
      for f in $glob; do
        [ -f "$f" ] && [ "$(stat -c %s "$f")" -le 65536 ] && cp -n "$f" "$corpus/"
      done
    done <fuzz/seed-fixtures
  fi
  dict=()
  if [ -f "fuzz/dict/$t.dict" ]; then
    dict=("-dict=$here/fuzz/dict/$t.dict")
  elif [ -f fuzz/fuzz.dict ]; then
    dict=("-dict=$here/fuzz/fuzz.dict")
  fi
  echo "::group::fuzz $crate $t ($(find "$corpus" -type f | wc -l) seeds, ${secs}s)"
  # shellcheck disable=SC2086 # word-split the flags
  cargo fuzz run ${FUZZ_BUILD_FLAGS:-} "$t" "$here/$corpus" -- -max_total_time="$secs" -rss_limit_mb="$rss" \
    -malloc_limit_mb="$rss" -timeout=20 -print_final_stats=1 "${dict[@]}" "$@"
  echo "::endgroup::"
done
