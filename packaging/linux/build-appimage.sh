#!/usr/bin/env bash
# Builds Specular-<version>-x86_64.AppImage.
#
#   packaging/linux/build-appimage.sh [--bin PATH] [--host PATH] [--out DIR] [--suffix S] [--no-split] [--no-validate]
#
# --bin   the specular binary to package. Default: the BOLT or PGO binary from tools/release/pgo.sh
#         (target/pgo/dist/specular[.bolt]) if present, otherwise `cargo build --release`.
# --host  the sandboxed decode host (HEIC, RAR/CBR), installed next to the viewer in usr/bin.
#         Default: specular-decode-host next to --bin, otherwise `cargo build --release`.
# --out   output directory (default target/dist).
# --suffix appended to the file names, e.g. -debug (build.sh uses it for debug builds).
# --no-split keeps the debug info inside the binary (debug builds) instead of a separate .debug file.
#
# Tools: appimagetool and the type-2 runtime are taken from PATH / target/tools, or downloaded
# into target/tools (never ~/.cache or /tmp). appimagetool is extracted once and run without FUSE.
#
# Portability: the AppImage needs a glibc at least as new as the build machine's. Release builds
# must therefore run on the oldest supported distribution (the CI job uses ubuntu-22.04). GPU
# drivers, libvulkan, libwayland and libxkbcommon are loaded from the host at run time (wgpu and
# winit dlopen them), as they must match the host's drivers; nothing else is bundled.
#
# Only the files listed below go into the AppDir. Nothing from tools/corpus is ever packaged.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
HERE="$ROOT/packaging/linux"
APP_ID=app.specular.Specular
TOOLS="$ROOT/target/tools"
OUT="$ROOT/target/dist"
BIN=
HOST_BIN=
SUFFIX=
SPLIT=1
# SPECULAR_APPIMAGE_VALIDATE=0 does what --no-validate does (for builds through build.sh).
VALIDATE=${SPECULAR_APPIMAGE_VALIDATE:-1}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin) BIN=$(realpath "$2"); shift ;;
        --host) HOST_BIN=$(realpath "$2"); shift ;;
        --out) OUT=$(realpath -m "$2"); shift ;;
        --suffix) SUFFIX=$2; shift ;;
        --no-split) SPLIT=0 ;;
        --no-validate) VALIDATE=0 ;;
        -h|--help) sed -n '2,24p' "$0"; exit 0 ;;
        *) echo "build-appimage.sh: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done
export TMPDIR="${TMPDIR:-$ROOT/target/tmp}"
WORK="$ROOT/target/appimage"
mkdir -p "$OUT" "$TOOLS" "$TMPDIR" "$WORK"
log() { printf '[appimage] %s\n' "$*"; }

VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml")
ARCH=x86_64

# --- binary -------------------------------------------------------------------------------------
if [[ -z "$BIN" ]]; then
    for c in "$ROOT/target/pgo/dist/specular.bolt" "$ROOT/target/pgo/dist/specular"; do
        [[ -x "$c" ]] && { BIN=$c; break; }
    done
fi
if [[ -z "$BIN" ]]; then
    log "no PGO binary found: building plain release"
    (cd "$ROOT" && cargo build --locked --release --features specular/asm -p specular --bin specular)
    BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/specular"
fi
log "binary: $BIN"
if [[ -z "$HOST_BIN" && -x "$(dirname "$BIN")/specular-decode-host" ]]; then
    HOST_BIN="$(dirname "$BIN")/specular-decode-host"
fi
if [[ -z "$HOST_BIN" ]]; then
    log "no decode host next to the binary: building it (release)"
    (cd "$ROOT" && cargo build --locked --release -p specular-decode-host --bin specular-decode-host)
    HOST_BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/specular-decode-host"
fi
log "decode host: $HOST_BIN"

# --- validation ---------------------------------------------------------------------------------
if [[ $VALIDATE == 1 ]]; then
    if command -v desktop-file-validate >/dev/null; then
        desktop-file-validate "$HERE/$APP_ID.desktop"
        log "desktop-file-validate: ok"
    else
        log "desktop-file-validate not installed: skipped"
    fi
    if command -v appstreamcli >/dev/null; then
        appstreamcli validate --no-net "$HERE/$APP_ID.metainfo.xml"
    else
        log "appstreamcli not installed: skipped"
    fi
fi

# --- tools --------------------------------------------------------------------------------------
fetch() { # fetch URL DEST
    [[ -s "$2" ]] && return 0
    log "downloading $1"
    curl -fsSL --retry 3 -o "$2.part" "$1" && mv "$2.part" "$2"
}
APPIMAGETOOL_RUN="$TOOLS/appimagetool/AppRun"
if [[ ! -x "$APPIMAGETOOL_RUN" ]]; then
    src=$(command -v appimagetool || true)
    if [[ -z "$src" ]]; then
        src="$TOOLS/appimagetool-x86_64.AppImage"
        fetch https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage "$src"
        chmod +x "$src"
    fi
    log "extracting appimagetool from $src"
    rm -rf "$TOOLS/appimagetool" "$TOOLS/squashfs-root"
    (cd "$TOOLS" && "$src" --appimage-extract >/dev/null && mv squashfs-root appimagetool)
fi
RUNTIME="$TOOLS/runtime-$ARCH"
fetch "https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-$ARCH" "$RUNTIME"

# --- AppDir -------------------------------------------------------------------------------------
APPDIR="$WORK/AppDir"
rm -rf "$APPDIR"
install -Dm755 "$BIN" "$APPDIR/usr/bin/specular"
# The viewer looks for the decode host next to its own executable.
install -Dm755 "$HOST_BIN" "$APPDIR/usr/bin/specular-decode-host"
# Separate the line tables (kept for symbolising crash reports) from the shipped binaries.
if [[ $SPLIT == 1 ]]; then
    OBJCOPY=$(command -v llvm-objcopy || command -v objcopy)
    for b in specular specular-decode-host; do
        "$OBJCOPY" --only-keep-debug "$APPDIR/usr/bin/$b" "$OUT/$b-$VERSION-$ARCH$SUFFIX.debug"
        "$OBJCOPY" --strip-all --add-gnu-debuglink="$OUT/$b-$VERSION-$ARCH$SUFFIX.debug" "$APPDIR/usr/bin/$b"
    done
fi

install -Dm644 "$HERE/$APP_ID.desktop" "$APPDIR/usr/share/applications/$APP_ID.desktop"
install -Dm644 "$HERE/$APP_ID.metainfo.xml" "$APPDIR/usr/share/metainfo/$APP_ID.appdata.xml"
install -Dm644 "$HERE/$APP_ID.mime.xml" "$APPDIR/usr/share/mime/packages/$APP_ID.xml"
# For desktop integrators that copy usr/share (it needs `specular` on PATH to work).
install -Dm644 "$HERE/specular.thumbnailer" "$APPDIR/usr/share/thumbnailers/specular.thumbnailer"
install -Dm644 "$ROOT/packaging/icons/specular.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
for s in 16 32 48 64 128 256 512; do
    install -Dm644 "$ROOT/packaging/icons/specular-$s.png" "$APPDIR/usr/share/icons/hicolor/${s}x${s}/apps/$APP_ID.png"
done
install -Dm644 "$ROOT/packaging/THIRD-PARTY.md" "$APPDIR/usr/share/doc/specular/THIRD-PARTY.md"
for f in LICENSE-MIT LICENSE-APACHE; do install -Dm644 "$ROOT/$f" "$APPDIR/usr/share/doc/specular/$f"; done

# AppImage top level: desktop file, icon, .DirIcon, AppRun.
cp "$HERE/$APP_ID.desktop" "$APPDIR/"
cp "$ROOT/packaging/icons/specular.svg" "$APPDIR/$APP_ID.svg"
cp "$ROOT/packaging/icons/specular-256.png" "$APPDIR/.DirIcon"
cat >"$APPDIR/AppRun" <<'EOF'
#!/bin/sh
# Specular AppImage entry point. Only the viewer and its decode host are bundled; system
# libraries (and libheif, if installed) come from the host.
HERE=$(dirname "$(readlink -f "$0")")
exec "$HERE/usr/bin/specular" "$@"
EOF
chmod 755 "$APPDIR/AppRun"

# --- pack ---------------------------------------------------------------------------------------
IMG="$OUT/Specular-$VERSION-$ARCH$SUFFIX.AppImage"
rm -f "$IMG"
ARCH=$ARCH VERSION=$VERSION "$APPIMAGETOOL_RUN" --no-appstream --runtime-file "$RUNTIME" \
    --comp zstd "$APPDIR" "$IMG" >"$WORK/appimagetool.log" 2>&1 \
    || { tail -30 "$WORK/appimagetool.log"; exit 1; }
(cd "$OUT" && sha256sum "$(basename "$IMG")" >"$(basename "$IMG").sha256")
log "built $IMG ($(du -h "$IMG" | cut -f1))"
