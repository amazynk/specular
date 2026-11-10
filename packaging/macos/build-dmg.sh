#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) Specular.app and .dmg. UNTESTED: never run (the app itself builds and runs from source on Apple Silicon, 2026-10-06).
# Runs on macOS 13+ with Xcode command-line tools. Normally called by ./build.sh.
#
#   packaging/macos/build-dmg.sh [--debug] [--out DIR] [--sign] [--bin-only]
#
# --debug     dev-profile build; artefact names get a -debug suffix.
# --out DIR   output directory (default target/dist).
# --bin-only  stop after the universal binaries (copied to DIR/specular[-debug] and
#             DIR/specular-decode-host).
# --sign      Developer ID signature, notarisation and stapling. Needs (CI secrets):
#   SPECULAR_SIGN_IDENTITY   "Developer ID Application: <Name> (<TEAMID>)", in the keychain
#   SPECULAR_NOTARY_PROFILE  a `xcrun notarytool store-credentials` profile name, or else
#   APPLE_ID, APPLE_TEAM_ID (or TEAM_ID), APPLE_APP_PASSWORD (app-specific password)
# Without --sign the app gets an ad-hoc signature and is not notarised (Gatekeeper will warn).
#
# Bundle layout: Contents/MacOS/specular (the viewer) and Contents/Helpers/specular-decode-host
# (the sandboxed decode host; the viewer looks for it in ../Helpers, specular-sandbox's
# Host::exe). The host is signed first, with its own identifier and entitlements
# (entitlements-decode-host.plist), then the app around it (no --deep, Apple TN2206).
#
# The x86_64 slice is built for x86-64-v3 (AVX2; .cargo/config.toml). Every Intel Mac that runs
# macOS 13 (2017 and later) has AVX2.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
HERE="$ROOT/packaging/macos"
OUT="$ROOT/target/dist"
TGT="${CARGO_TARGET_DIR:-$ROOT/target}"
PROFILE=release
SUFFIX=
SIGN=0
BIN_ONLY=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug) PROFILE=debug; SUFFIX=-debug ;;
        --out) OUT=$2; shift ;;
        --sign) SIGN=1 ;;
        --bin-only) BIN_ONLY=1 ;;
        -h|--help) sed -n '2,23p' "$0"; exit 0 ;;
        *) echo "build-dmg.sh: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done
cd "$ROOT"
VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml)
mkdir -p "$OUT"
log() { printf '[dmg] %s\n' "$*"; }
die() { printf 'build-dmg.sh: %s\n' "$*" >&2; exit 1; }
for t in lipo dsymutil codesign hdiutil iconutil plutil; do
    command -v "$t" >/dev/null || die "$t not found: install the Xcode command-line tools (xcode-select --install)"
done
if [[ $SIGN == 1 ]]; then
    [[ -n "${SPECULAR_SIGN_IDENTITY:-}" ]] || die "--sign needs SPECULAR_SIGN_IDENTITY (a Developer ID Application identity in the keychain)"
    APPLE_TEAM_ID=${APPLE_TEAM_ID:-${TEAM_ID:-}}
    if [[ -z "${SPECULAR_NOTARY_PROFILE:-}" ]] && [[ -z "${APPLE_ID:-}" || -z "$APPLE_TEAM_ID" || -z "${APPLE_APP_PASSWORD:-}" ]]; then
        die "--sign needs SPECULAR_NOTARY_PROFILE, or APPLE_ID + APPLE_TEAM_ID (or TEAM_ID) + APPLE_APP_PASSWORD, for notarisation"
    fi
fi

# --- universal binary ---------------------------------------------------------------------------
# PGO for macOS would need tools/release/pgo.sh's steps per target on matching hardware; not done.
CARGO_PROFILE=()
[[ $PROFILE == release ]] && CARGO_PROFILE=(--release --features specular/asm)   # rav1d/rav1e assembly: needs nasm
for t in aarch64-apple-darwin x86_64-apple-darwin; do
    rustup target add "$t" >/dev/null
    cargo build --locked ${CARGO_PROFILE[@]+"${CARGO_PROFILE[@]}"} --target "$t" -p specular --bin specular \
        -p specular-decode-host --bin specular-decode-host
done
WORK="$TGT/macos$SUFFIX"
HOST_EXE=specular-decode-host
rm -rf "$WORK"; mkdir -p "$WORK"
for b in specular "$HOST_EXE"; do
    lipo -create -output "$WORK/$b" \
        "$TGT/aarch64-apple-darwin/$PROFILE/$b" "$TGT/x86_64-apple-darwin/$PROFILE/$b"
done
if [[ $BIN_ONLY == 1 ]]; then
    cp "$WORK/specular" "$OUT/specular$SUFFIX"
    # Next to the viewer, where it looks first.
    cp "$WORK/$HOST_EXE" "$OUT/$HOST_EXE"
    log "built $OUT/specular$SUFFIX and $OUT/$HOST_EXE (universal)"
    exit 0
fi
# Keep symbols for crash reports, ship stripped binaries (debug builds keep theirs).
if [[ $PROFILE == release ]]; then
    dsymutil "$WORK/specular" -o "$OUT/Specular-$VERSION$SUFFIX.dSYM"
    dsymutil "$WORK/$HOST_EXE" -o "$OUT/$HOST_EXE-$VERSION$SUFFIX.dSYM"
    strip -x "$WORK/specular" "$WORK/$HOST_EXE"
fi

# --- bundle -------------------------------------------------------------------------------------
APP="$WORK/Specular.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Helpers" "$APP/Contents/Resources"
cp "$WORK/specular" "$APP/Contents/MacOS/specular"
cp "$WORK/$HOST_EXE" "$APP/Contents/Helpers/$HOST_EXE"
sed "s/@VERSION@/$VERSION/g" "$HERE/Info.plist" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
cp packaging/THIRD-PARTY.md "$APP/Contents/Resources/"

ICONSET="$WORK/Specular.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    cp "packaging/icons/specular-$s.png" "$ICONSET/icon_${s}x${s}.png"
    d=$((s * 2))
    if [[ -f "packaging/icons/specular-$d.png" ]]; then
        cp "packaging/icons/specular-$d.png" "$ICONSET/icon_${s}x${s}@2x.png"
    else
        sips -z "$d" "$d" "packaging/icons/specular-512.png" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
    fi
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Specular.icns"

# --- sign ---------------------------------------------------------------------------------------
# Inside out: the helper (a bare Mach-O, so it gets an explicit identifier), then the app.
NOTARIZE=$SIGN
HELPER="$APP/Contents/Helpers/$HOST_EXE"
if [[ $SIGN == 1 ]]; then
    codesign --force --timestamp --options runtime --identifier app.specular.Specular.decode-host \
        --entitlements "$HERE/entitlements-decode-host.plist" --sign "$SPECULAR_SIGN_IDENTITY" "$HELPER"
    codesign --force --timestamp --options runtime --entitlements "$HERE/entitlements.plist" \
        --sign "$SPECULAR_SIGN_IDENTITY" "$APP"
else
    log "no --sign: ad-hoc signature, no notarisation"
    codesign --force --identifier app.specular.Specular.decode-host --sign - "$HELPER"
    codesign --force --sign - "$APP"
fi
codesign --verify --strict --verbose=2 "$HELPER"
codesign --verify --strict --verbose=2 "$APP"

# --- dmg ----------------------------------------------------------------------------------------
DMG="$OUT/Specular-$VERSION-universal$SUFFIX.dmg"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "Specular $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG"
[[ $SIGN == 1 ]] && codesign --timestamp --sign "$SPECULAR_SIGN_IDENTITY" "$DMG"

# --- notarise -----------------------------------------------------------------------------------
if [[ $NOTARIZE == 1 ]]; then
    if [[ -n "${SPECULAR_NOTARY_PROFILE:-}" ]]; then
        xcrun notarytool submit "$DMG" --keychain-profile "$SPECULAR_NOTARY_PROFILE" --wait
    else
        xcrun notarytool submit "$DMG" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
            --password "$APPLE_APP_PASSWORD" --wait
    fi
    xcrun stapler staple "$DMG"
    xcrun stapler validate "$DMG"
    spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG"
fi
shasum -a 256 "$DMG" >"$DMG.sha256"
log "built $DMG"
