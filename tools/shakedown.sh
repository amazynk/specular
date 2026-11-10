#!/usr/bin/env bash
# Real-world shakedown: every decoder over every corpus file under
# tools/corpus/* except tools/corpus/real, one child process per file, with a per-file time
# budget and a resident-memory limit. Writes files.tsv, summary.tsv, summary.json and top.tsv
# (the 30 slowest files and largest peaks) under target/shakedown/<label> so pass rates can be
# compared over time.
#
#   tools/shakedown.sh [--corpus DIR] [--label NAME] [--thumbnails] [--compare LABEL|DIR]
#                      [harness options...]
#
# --thumbnails also runs the desktop thumbnailer (`specular --thumbnail IN OUT 256`, built
# here) on every file. --compare names a previous run (a label under target/shakedown or a
# folder) and writes compare.tsv: status changes, files twice as slow or twice as large.
# Harness options (crates/specular-thumbs/examples/shakedown.rs): --jobs N (4), --timeout SECS
# (60), --rss-limit MIB (8192), --exclude SUBSTR, --max-files N, --out DIR, --thumbnail-size PX.
# The label defaults to the date and the commit. Dev profile (no release build); decoders run
# at the workspace's dev optimisation levels, so timings are not the release app's. Peak memory
# and CPU time come from GNU time (/usr/bin/time) when it is installed.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
corpus=$root/tools/corpus
label="$(date +%Y%m%d-%H%M)-$(git -C "$root" rev-parse --short HEAD 2>/dev/null || echo nogit)"
thumbnails=0
compare=
args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --corpus) corpus=$2; shift 2 ;;
    --label) label=$2; shift 2 ;;
    --thumbnails) thumbnails=1; shift ;;
    --compare) compare=$2; shift 2 ;;
    *) args+=("$1"); shift ;;
  esac
done

roots=()
for d in "$corpus"/*/; do
  d=${d%/}
  # The user's real-world files are never read by sweeps.
  [ "$(basename "$d")" = real ] && continue
  roots+=("$d")
done
[ ${#roots[@]} -gt 0 ] || { echo "shakedown: no corpus folders under $corpus" >&2; exit 1; }

results=$root/target/shakedown
target=${CARGO_TARGET_DIR:-$root/target}
cargo build -q -p specular-thumbs --example shakedown
if [ "$thumbnails" = 1 ]; then
  cargo build -q -p specular --bin specular
  args+=(--thumbnail-exe "$target/debug/specular")
fi
if [ -n "$compare" ]; then
  [ -e "$compare" ] || compare=$results/$compare
  args+=(--compare "$compare")
fi
exec "$target/debug/examples/shakedown" --label "$label" --out "$results/$label" \
  --exclude "/tools/corpus/real/" --exclude "/tools/corpus/out/real/" \
  "${args[@]}" "${roots[@]}"
