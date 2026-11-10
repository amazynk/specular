#!/usr/bin/env bash
# Builds specular_<version>_amd64.deb and specular-<version>-1.x86_64.rpm from prebuilt binaries.
#
#   packaging/linux/build-deb-rpm.sh [--bin PATH] [--host PATH] [--out DIR] [--suffix S]
#                                    [--no-split] [--deb-only|--rpm-only]
#
# --bin    the specular binary. Default: target/pgo/dist/specular[.bolt] if present, otherwise
#          `cargo build --release`.
# --host   the sandboxed decode host. Default: specular-decode-host next to --bin, otherwise
#          `cargo build --release`.
# --out    output directory (default target/dist).
# --suffix appended to the file names, e.g. -debug (build.sh uses it for debug builds).
# --no-split keeps the debug info in the binaries instead of separate .debug files.
#
# Layout (both packages):
#   /usr/bin/specular
#   /usr/libexec/specular/specular-decode-host   (the viewer also looks in ../libexec/specular: the
#                                               host is not a user command, so it stays off PATH,
#                                               as FHS 3.0, Debian policy 9.1.1 and Fedora want)
#   /usr/libexec/specular/libspecular-occt.so    (when built: the OpenCASCADE bridge, next to the
#                                               host, which loads it at run time)
#   /usr/libexec/specular/libspecular-czi.so     (when built: the libCZI bridge, next to the host,
#                                               which loads it at run time)
#   /usr/share/thumbnailers/specular.thumbnailer
#   /usr/share/{applications,metainfo,mime/packages,icons/hicolor,doc/specular}/...
# Desktop, MIME and icon caches are refreshed by the distributions' own triggers (dpkg triggers
# of desktop-file-utils/shared-mime-info/hicolor; RPM file triggers), so there are no scripts.
#
# Dependencies: the .deb's come from dpkg-shlibdeps when available (otherwise a fixed list), the
# .rpm's from rpmbuild's automatic ELF scan. libheif (HEIC) is a recommendation only: it is
# loaded at run time by the decode host, and the viewer reports HEIC as "not installed" without it.
# OpenCASCADE (STEP, IGES) likewise: the bridge links the build machine's OCCT, but it is left out
# of the dependency scan (dpkg-shlibdeps is not given it; rpm excludes libTK*), so OCCT stays
# optional and STEP/IGES say "needs OpenCASCADE" without it.
# libCZI (Zeiss CZI) likewise: the bridge links the build machine's libCZI, but it is left out of
# the dependency scan (dpkg-shlibdeps is not given it; rpm excludes liblibCZI), so libCZI stays
# optional and CZI files say "need libCZI" without it.
#
# Tools: dpkg-deb (+ fakeroot unless run as root) for the .deb, rpmbuild for the .rpm; a missing
# tool skips that package with a message. All work files live in target/package/deb-rpm.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
HERE="$ROOT/packaging/linux"
APP_ID=app.specular.Specular
OUT="$ROOT/target/dist"
BIN=
HOST_BIN=
SUFFIX=
SPLIT=1
DEB=1
RPM=1
while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin) BIN=$(realpath "$2"); shift ;;
        --host) HOST_BIN=$(realpath "$2"); shift ;;
        --out) OUT=$(realpath -m "$2"); shift ;;
        --suffix) SUFFIX=$2; shift ;;
        --no-split) SPLIT=0 ;;
        --deb-only) RPM=0 ;;
        --rpm-only) DEB=0 ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "build-deb-rpm.sh: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done
export TMPDIR="${TMPDIR:-$ROOT/target/tmp}"
WORK="$ROOT/target/package/deb-rpm"
mkdir -p "$OUT" "$TMPDIR" "$WORK"
log() { printf '[deb-rpm] %s\n' "$*"; }

VERSION=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml")
LICENSE=$(sed -n '/^\[workspace.package\]/,/^\[/s/^license *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml")
HOMEPAGE=$(sed -n '/^\[workspace.package\]/,/^\[/s/^repository *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml")
SUMMARY="Fast image viewer for large photos, HDR, RAW and comic archives"

# --- binaries -----------------------------------------------------------------------------------
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
if [[ -z "$HOST_BIN" && -x "$(dirname "$BIN")/specular-decode-host" ]]; then
    HOST_BIN="$(dirname "$BIN")/specular-decode-host"
fi
if [[ -z "$HOST_BIN" ]]; then
    log "no decode host next to the binary: building it (release)"
    (cd "$ROOT" && cargo build --locked --release -p specular-decode-host --bin specular-decode-host)
    HOST_BIN="${CARGO_TARGET_DIR:-$ROOT/target}/release/specular-decode-host"
fi
log "binary: $BIN"
log "decode host: $HOST_BIN"

# --- staged tree (shared by both packages) ------------------------------------------------------
STAGE="$WORK/root"
rm -rf "$STAGE"
install -Dm755 "$BIN" "$STAGE/usr/bin/specular"
install -Dm755 "$HOST_BIN" "$STAGE/usr/libexec/specular/specular-decode-host"
OCCT_BRIDGE="$(dirname "$HOST_BIN")/libspecular-occt.so"
if [[ -f "$OCCT_BRIDGE" ]]; then
    install -Dm755 "$OCCT_BRIDGE" "$STAGE/usr/libexec/specular/libspecular-occt.so"
    log "OpenCASCADE bridge: $OCCT_BRIDGE"
fi
CZI_BRIDGE="$(dirname "$HOST_BIN")/libspecular-czi.so"
if [[ -f "$CZI_BRIDGE" ]]; then
    install -Dm755 "$CZI_BRIDGE" "$STAGE/usr/libexec/specular/libspecular-czi.so"
    log "libCZI bridge: $CZI_BRIDGE"
fi
OBJCOPY=$(command -v llvm-objcopy || command -v objcopy)
for b in usr/bin/specular usr/libexec/specular/specular-decode-host; do
    n=$(basename "$b")
    if [[ $SPLIT == 1 ]]; then
        "$OBJCOPY" --only-keep-debug "$STAGE/$b" "$OUT/$n-$VERSION-x86_64$SUFFIX.debug"
        "$OBJCOPY" --strip-all --add-gnu-debuglink="$OUT/$n-$VERSION-x86_64$SUFFIX.debug" "$STAGE/$b"
    fi
done
install -Dm644 "$HERE/specular.thumbnailer" "$STAGE/usr/share/thumbnailers/specular.thumbnailer"
install -Dm644 "$HERE/$APP_ID.desktop" "$STAGE/usr/share/applications/$APP_ID.desktop"
install -Dm644 "$HERE/$APP_ID.metainfo.xml" "$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
install -Dm644 "$HERE/$APP_ID.mime.xml" "$STAGE/usr/share/mime/packages/$APP_ID.xml"
install -Dm644 "$ROOT/packaging/icons/specular.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
for s in 16 32 48 64 128 256 512; do
    install -Dm644 "$ROOT/packaging/icons/specular-$s.png" "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps/$APP_ID.png"
done
install -Dm644 "$ROOT/packaging/THIRD-PARTY.md" "$STAGE/usr/share/doc/specular/THIRD-PARTY.md"
for f in LICENSE-MIT LICENSE-APACHE; do install -Dm644 "$ROOT/$f" "$STAGE/usr/share/doc/specular/$f"; done
FILES=$(cd "$STAGE" && find . -type f | sed 's#^\.##' | sort)

# --- .deb ---------------------------------------------------------------------------------------
build_deb() {
    local d="$WORK/deb" deb="$OUT/specular_${VERSION}${SUFFIX}_amd64.deb" depends=
    rm -rf "$d"; cp -a "$STAGE" "$d"; mkdir -p "$d/DEBIAN"
    if command -v dpkg-shlibdeps >/dev/null; then
        # dpkg-shlibdeps wants a debian/control next to it.
        local sd="$WORK/shlibdeps"
        rm -rf "$sd"; mkdir -p "$sd/debian"
        printf 'Source: specular\n\nPackage: specular\nArchitecture: amd64\n' >"$sd/debian/control"
        depends=$(cd "$sd" && dpkg-shlibdeps -O "$d/usr/bin/specular" \
            "$d/usr/libexec/specular/specular-decode-host" 2>/dev/null | sed -n 's/^shlibs:Depends=//p') || depends=
    fi
    [[ -n "$depends" ]] || depends="libc6 (>= 2.35), libgcc-s1, libstdc++6, libfontconfig1, libfreetype6"
    cat >"$d/DEBIAN/control" <<EOF
Package: specular
Version: $VERSION
Architecture: amd64
Maintainer: The Specular authors <help@amazynk.com>
Installed-Size: $(du -sk --exclude=DEBIAN "$d" | cut -f1)
Depends: $depends
Recommends: libheif1, libvulkan1, mesa-vulkan-drivers | vulkan-icd
Section: graphics
Priority: optional
Homepage: $HOMEPAGE
Description: $SUMMARY
 Specular opens and flips through large photos instantly. HEIC images and RAR/CBR
 archives are decoded in a separate, sandboxed helper process (Landlock and
 seccomp); HEIC support uses the system's libheif when it is installed.
 .
 Licence: $LICENSE
EOF
    # Debian policy 12.5: a copyright file (machine-readable format).
    cat >"$d/usr/share/doc/specular/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Specular
Source: $HOMEPAGE

Files: *
Copyright: The Specular authors
License: $LICENSE
 See $HOMEPAGE; bundled third-party code is listed in
 /usr/share/doc/specular/THIRD-PARTY.md.
EOF
    (cd "$d" && find usr -type f -exec md5sum {} + | sort -k2) >"$d/DEBIAN/md5sums"
    local fr=()
    [[ $(id -u) == 0 ]] || fr=(fakeroot)
    rm -f "$deb"
    "${fr[@]}" dpkg-deb --root-owner-group -Zxz --build "$d" "$deb" >/dev/null
    (cd "$OUT" && sha256sum "$(basename "$deb")" >"$(basename "$deb").sha256")
    log "built $deb ($(du -h "$deb" | cut -f1))"
}

# --- .rpm ---------------------------------------------------------------------------------------
build_rpm() {
    local top="$WORK/rpm" rver=${VERSION//-/_} rpm
    rm -rf "$top"; mkdir -p "$top"/{BUILD,RPMS,SPECS,SRPMS,SOURCES,tmp}
    {
        cat <<EOF
# Generated by packaging/linux/build-deb-rpm.sh: packages prebuilt binaries.
%global debug_package %{nil}
%global __os_install_post %{nil}
# OpenCASCADE and libCZI are optional (their bridges are loaded at run time): no requirement
# on their libraries.
%global __requires_exclude ^(libTK.*|liblibCZI.*)$
Name:           specular
Version:        $rver
Release:        1
Summary:        $SUMMARY
License:        $LICENSE
URL:            $HOMEPAGE
ExclusiveArch:  x86_64
Recommends:     libheif
Recommends:     (mesa-vulkan-drivers or vulkan-loader)

%description
Specular opens and flips through large photos instantly. HEIC images and RAR/CBR
archives are decoded in a separate, sandboxed helper process (Landlock and
seccomp); HEIC support uses the system's libheif when it is installed.

%install
mkdir -p %{buildroot}
cp -a $STAGE/. %{buildroot}/

%files
%defattr(-,root,root,-)
EOF
        echo "$FILES"
    } >"$top/SPECS/specular.spec"
    rpmbuild --quiet -bb --define "_topdir $top" --define "_tmppath $top/tmp" \
        --define "_build_id_links none" "$top/SPECS/specular.spec" >"$WORK/rpmbuild.log" 2>&1 \
        || { tail -30 "$WORK/rpmbuild.log"; return 1; }
    rpm=$(find "$top/RPMS" -name '*.rpm' | head -1)
    local dst="$OUT/specular-$rver-1.x86_64$SUFFIX.rpm"
    mv "$rpm" "$dst"
    (cd "$OUT" && sha256sum "$(basename "$dst")" >"$(basename "$dst").sha256")
    log "built $dst ($(du -h "$dst" | cut -f1))"
}

rc=0
if [[ $DEB == 1 ]]; then
    if command -v dpkg-deb >/dev/null && { [[ $(id -u) == 0 ]] || command -v fakeroot >/dev/null; }; then
        build_deb || rc=1
    else
        log "dpkg-deb or fakeroot not found: .deb skipped (apt install dpkg-dev fakeroot)"
        rc=1
    fi
fi
if [[ $RPM == 1 ]]; then
    if command -v rpmbuild >/dev/null; then
        build_rpm || rc=1
    else
        log "rpmbuild not found: .rpm skipped (apt install rpm / dnf install rpm-build)"
        rc=1
    fi
fi
exit $rc
