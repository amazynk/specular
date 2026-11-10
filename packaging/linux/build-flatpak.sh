#!/usr/bin/env bash
# Builds a Specular Flatpak bundle into target/dist.
#
#   packaging/linux/build-flatpak.sh --local [--bin PATH] [--host PATH]   # prebuilt binaries (fast)
#   packaging/linux/build-flatpak.sh                        # from source, offline, Flathub style
#   common options: [--out DIR] [--suffix S]   (S is appended to the bundle name, e.g. -debug)
#
# --local uses app.specular.Specular.local.yml and the binary from tools/release/pgo.sh
# (target/pgo/dist/specular[.bolt]) unless --bin is given, and the decode host given by --host,
# else the one next to that binary, else a fresh `cargo build --release` of it.
# The from-source build needs org.freedesktop.Sdk//25.08 and
# org.freedesktop.Sdk.Extension.rust-nightly//25.08 (installed with
# `flatpak install flathub ...`), plus python3 with `aiohttp` and `tomlkit` for
# flatpak-cargo-generator, which is downloaded into target/tools.
#
# All state (flatpak-builder cache, build dir, repo) lives in target/flatpak.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
HERE="$ROOT/packaging/linux"
APP_ID=app.specular.Specular
WORK="$ROOT/target/flatpak"
OUT="$ROOT/target/dist"
SUFFIX=
LOCAL=0
BIN=
HOST_BIN=
while [[ $# -gt 0 ]]; do
    case "$1" in
        --local) LOCAL=1 ;;
        --bin) BIN=$(realpath "$2"); shift ;;
        --host) HOST_BIN=$(realpath "$2"); shift ;;
        --out) OUT=$(realpath -m "$2"); shift ;;
        --suffix) SUFFIX=$2; shift ;;
        -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
        *) echo "build-flatpak.sh: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done
export TMPDIR="${TMPDIR:-$ROOT/target/tmp}"
mkdir -p "$WORK" "$OUT" "$TMPDIR"
log() { printf '[flatpak] %s\n' "$*"; }
VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml")

if [[ $LOCAL == 1 ]]; then
    if [[ -z "$BIN" ]]; then
        for c in "$ROOT/target/pgo/dist/specular.bolt" "$ROOT/target/pgo/dist/specular"; do
            [[ -x "$c" ]] && { BIN=$c; break; }
        done
    fi
    [[ -n "$BIN" ]] || { echo "no binary: run tools/release/pgo.sh or pass --bin" >&2; exit 1; }
    OBJCOPY=$(command -v llvm-objcopy || command -v objcopy)
    "$OBJCOPY" --strip-all "$BIN" "$WORK/specular"
    if [[ -z "$HOST_BIN" && -x "$(dirname "$BIN")/specular-decode-host" ]]; then
        HOST_BIN="$(dirname "$BIN")/specular-decode-host"
    fi
    if [[ -z "$HOST_BIN" ]]; then
        log "no decode host next to the binary: building it (release)"
        (cd "$ROOT" && cargo build --locked --release -p specular-decode-host --bin specular-decode-host)
        HOST_BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/specular-decode-host"
    fi
    "$OBJCOPY" --strip-all "$HOST_BIN" "$WORK/specular-decode-host"
    MANIFEST="$HERE/$APP_ID.local.yml"
else
    GEN="$ROOT/target/tools/flatpak-cargo-generator.py"
    if [[ ! -s "$GEN" ]]; then
        curl -fsSL -o "$GEN" https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak_cargo_generator.py
    fi
    python3 -I "$GEN" "$ROOT/Cargo.lock" -o "$WORK/cargo-sources.json"
    # The manifest refers to the source tree as ../.. and to cargo-sources.json next to itself:
    # use a copy in $WORK with absolute paths.
    sed "s#path: \.\./\.\.#path: $ROOT#" "$HERE/$APP_ID.yml" >"$WORK/$APP_ID.yml"
    MANIFEST="$WORK/$APP_ID.yml"
fi

# OSTree refuses to write when less than 3% of the volume would stay free (57 GB on a 1.9 TB
# disk). Use an absolute reserve for this throw-away repo instead.
if [[ ! -f "$WORK/repo/config" ]]; then
    command -v ostree >/dev/null || { echo "ostree not found: install it (apt install ostree)" >&2; exit 1; }
    ostree init --mode=archive-z2 --repo="$WORK/repo"
fi
ostree config --repo="$WORK/repo" set core.min-free-space-size 1GB

log "flatpak-builder $(basename "$MANIFEST")"
flatpak-builder --force-clean --disable-rofiles-fuse --state-dir="$WORK/state" \
    --repo="$WORK/repo" "$WORK/build" "$MANIFEST" >"$WORK/builder.log" 2>&1 \
    || { tail -30 "$WORK/builder.log"; exit 1; }
BUNDLE="$OUT/Specular-$VERSION-$(uname -m)$SUFFIX.flatpak"
flatpak build-bundle "$WORK/repo" "$BUNDLE" "$APP_ID"
(cd "$OUT" && sha256sum "$(basename "$BUNDLE")" >"$(basename "$BUNDLE").sha256")
log "built $BUNDLE ($(du -h "$BUNDLE" | cut -f1))"
