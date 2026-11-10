#!/usr/bin/env bash
# cargo-deny over the whole workspace: licences, advisories, bans (duplicates warn), sources.
# Policy and every exception with its reason: deny.toml. Exits non-zero on any error.
#
#   tools/ci/deny.sh [--fetch] [--install] [SECTION...]
#
#   --fetch     download what the check reads first (network): the RustSec advisory snapshot
#               and any crate sources cargo metadata needs; without it everything runs offline
#   --install   install the pinned cargo-deny into target/tools if it is missing
#   SECTION     licenses, advisories, bans, sources (default: all four)
#
# The lockfile is never changed (--locked). Nothing is written outside the repo's target/
# (or $CARGO_TARGET_DIR): cargo-deny from PATH or <target>/tools/bin, the advisory snapshot in
# $SPECULAR_ADVISORY_DB (default <target>/advisory-dbs, see deny.toml).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"
DENY_VERSION=0.20.2

FETCH=0
INSTALL=0
SECTIONS=()
for a in "$@"; do
    case "$a" in
        --fetch) FETCH=1 ;;
        --install) INSTALL=1 ;;
        licenses | advisories | bans | sources) SECTIONS+=("$a") ;;
        -h | --help) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "deny.sh: unknown argument: $a" >&2; exit 2 ;;
    esac
done

TARGET=${CARGO_TARGET_DIR:-$ROOT/target}
export TMPDIR="${TMPDIR:-$TARGET/tmp}"
export SPECULAR_ADVISORY_DB="${SPECULAR_ADVISORY_DB:-$TARGET/advisory-dbs}"
mkdir -p "$TMPDIR" "$SPECULAR_ADVISORY_DB"

DENY=$(command -v cargo-deny || echo "$TARGET/tools/bin/cargo-deny")
if [[ ! -x "$DENY" ]]; then
    if ((INSTALL)); then
        CARGO_TARGET_DIR="$TARGET/tools-build" cargo install --locked --root "$TARGET/tools" \
            "cargo-deny@$DENY_VERSION"
        rm -rf "$TARGET/tools-build"
        DENY="$TARGET/tools/bin/cargo-deny"
    else
        echo "cargo-deny not found on PATH or in $TARGET/tools/bin; rerun with --install" >&2
        exit 1
    fi
fi

if ((FETCH)); then
    cargo fetch --locked
    "$DENY" --locked fetch db
elif [[ -z "$(ls -A "$SPECULAR_ADVISORY_DB" 2>/dev/null)" ]]; then
    echo "no advisory snapshot in $SPECULAR_ADVISORY_DB; rerun with --fetch" >&2
    exit 1
fi

# --offline: no fetches during the check itself, so a run is reproducible from the snapshot.
"$DENY" --offline --locked check --show-stats ${SECTIONS[@]+"${SECTIONS[@]}"}
