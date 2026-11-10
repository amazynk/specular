#!/usr/bin/env bash
# Build the specular-exr DWA and deep conformance corpus in
# $SPECULAR_CONFORMANCE_DIR/exr-dwa (default tools/corpus/conformance/exr-dwa; gitignored,
# nothing here is committed) with OpenEXR's own Python bindings (tools/corpus/exr-dwa.py).
# specular-exr's tests/conformance.rs and tests/robustness.rs read it and skip when absent.
#
# Usage: tools/corpus/exr-dwa.sh [--bench]
#   --bench also writes the 4k kloofendal file (tools/corpus/large/exr) as DWAA, DWAB, PIZ
#   and ZIP into target/exr-bench, for speed measurements.
#
# Needs python3 with venv. The OpenEXR and numpy wheels go into a private venv under
# target/corpus/exr-venv (pip --no-cache-dir, nothing outside target/). Run
# tools/corpus/fetch-conformance.sh first: the openexr-images files are re-encoded from
# there. Stops if free space is below MIN_FREE_GB (default 6).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CONF=${SPECULAR_CONFORMANCE_DIR:-$ROOT/tools/corpus/conformance}
VENV="$ROOT/target/corpus/exr-venv"
BENCH="$ROOT/target/exr-bench"
MIN_FREE_GB=${MIN_FREE_GB:-6}

die() { echo "exr-dwa: $*" >&2; exit 1; }

check_space() {
  local avail_kb
  avail_kb=$(df -Pk "$ROOT/target" | awk 'NR==2{print $4}')
  if (( avail_kb < MIN_FREE_GB * 1024 * 1024 )); then
    die "only $((avail_kb / 1024 / 1024)) GB free (< ${MIN_FREE_GB} GB); stopping"
  fi
}

main() {
  local args=()
  for a in "$@"; do
    case $a in
      -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
      --bench) args+=(--bench "$BENCH") ;;
      *) die "unknown argument $a" ;;
    esac
  done
  [[ -d $CONF/openexr-images ]] || die "$CONF/openexr-images missing (run tools/corpus/fetch-conformance.sh)"
  mkdir -p "$ROOT/target"
  check_space
  if [[ ! -x $VENV/bin/python ]]; then
    python3 -m venv "$VENV"
    XDG_CACHE_HOME="$ROOT/target/corpus" "$VENV/bin/pip" install -q --no-cache-dir 'OpenEXR>=3.3,<4' numpy
  fi
  TMPDIR="$ROOT/target/corpus" "$VENV/bin/python" -I "$ROOT/tools/corpus/exr-dwa.py" "$CONF" "${args[@]}"
  du -sh "$CONF/exr-dwa"
}
main "$@"
