#!/usr/bin/env bash
# Specular build entry point: compiles the viewer and, with --package, produces the installable
# packages for each platform. The packaging itself lives in packaging/<platform>/ and the PGO
# pipeline in tools/release/pgo.sh; this script only drives them.
#
# Runs on Linux, macOS (bash 3.2 or later) and Windows (Git Bash / MSYS2).
set -euo pipefail

usage() {
    # Section headings (unindented lines) in bold, the first word of option lines in cyan.
    local line
    usage_text | while IFS= read -r line; do
        case "$line" in
            '') echo ;;
            '  ./build.sh'*) printf '%s%s%s\n' "$C_CYAN" "$line" "$C_RESET" ;;
            ' '*)
                if [[ $line =~ ^(\ \ )([^ ].*[^ ])(\ \ +)(.*)$ ]] && [[ ${BASH_REMATCH[2]} != *'  '* ]]; then
                    printf '  %s%s%s%s%s\n' "$C_CYAN" "${BASH_REMATCH[2]}" "$C_RESET" "${BASH_REMATCH[3]}" "${BASH_REMATCH[4]}"
                else
                    printf '%s\n' "$line"
                fi ;;
            *) printf '%s%s%s\n' "$C_BOLD" "$line" "$C_RESET" ;;
        esac
    done
}
usage_text() {
    cat <<'EOF'
Usage: ./build.sh [release|debug] [linux|windows|macos|host|all] [options]
       ./build.sh pack [FILE]
       ./build.sh bench-gate [--bless] [perf-gate options]

Build mode (default: release)
  release        optimised build (fat LTO; see [profile.release] in Cargo.toml)
  debug          dev-profile build; packages get a -debug suffix

Platform (default: host)
  host           the platform this script runs on
  linux          x86_64 Linux (needs a Linux host)
  windows        x86_64 Windows: native under Git Bash/MSYS2, or cross-compiled from Linux/macOS
                 with cargo-xwin
  macos          universal arm64 + x86_64 (needs a Mac; elsewhere this is skipped, not an error)
  all            every platform this host can build, then a summary table

Source archive
  pack [FILE]    pack the project's code and docs (the working tree's files git tracks or would
                 track: no target/, corpora or other ignored files) into a .tgz, by default
                 /tmp/specular-<version>-src-<date>.tgz; nothing is built

Speed gate
  bench-gate     count the instructions of a fixed set of single-threaded decodes under
                 valgrind and compare them with tools/bench/perf-gate-baseline.json (fails at
                 +2 %), with the headline cases' budgets; --bless makes the new counts the
                 baseline. --suite app runs the viewer's scripted gates in a headless sway
                 instead (idle frames, frame causes, bytes read; needs sway and fincore),
                 --suite all both. Other options go to tools/bench/perf-gate.py (see its
                 --help). Linux only; the decode suite needs valgrind. --suite develop runs
                 Develop's GPU timing benches against
                 crates/specular-develop/benches/develop-baseline.json (+10 %; skipped on a
                 busy machine or without a GPU; --bless only when idle).

Options
  --package      also build the packages (without it, the binaries are only compiled and copied):
                   linux    .tar.gz, .AppImage, .flatpak, .deb, .rpm
                   windows  .zip, .msi (WiX v4), winget manifests (release only)
                   macos    .app inside a .dmg
  --pgo          profile-guided (and, if llvm-bolt is available, BOLT) optimised Linux binary via
                 tools/release/pgo.sh. Release only; ignored with a warning for windows/macos.
  --sign         sign the packages; implies --package.
                   windows  signtool with SPECULAR_SIGN_CERT_SHA1 (Windows host only)
                   macos    codesign + notarise: SPECULAR_SIGN_IDENTITY, and SPECULAR_NOTARY_PROFILE
                            or APPLE_ID + APPLE_TEAM_ID (or TEAM_ID) + APPLE_APP_PASSWORD
                   linux    nothing to sign (checksums are always written)
  --out DIR      output root (default: target/dist/<version>); files go to DIR/<platform>/
  --slice TARGET macos: build one half of the universal binary (aarch64-apple-darwin or
                 x86_64-apple-darwin) into DIR/macos/TARGET/, no package. The release workflow
                 builds the halves on two machines at once.
  --from DIR     macos: build nothing, join the halves --slice put in DIR/<target>/ (and package
                 them with --package)
  --no-color(s)  plain output, also with NO_COLOR or NO_COLORS set (FORCE_COLOR=1 keeps colour
                 when piped)
  -h, --help     this help

Environment
  CARGO_TARGET_DIR             cargo build directory (default: target/)
  TMPDIR                       kept if inside target/, otherwise set to target/tmp
  SPECULAR_ACCEPT_MSVC_LICENSE  set to 1 to let cargo-xwin download the MSVC CRT and Windows SDK
                               (Microsoft licence: https://go.microsoft.com/fwlink/?LinkId=2086102)
                               for cross-compiling to Windows
  SPECULAR_RELEASE_URL          base URL of the release downloads, used in the winget manifest

Examples
  ./build.sh                              release binary for this host
  ./build.sh debug linux                  debug binary in target/dist/<version>/linux/
  ./build.sh release linux --package --pgo
  ./build.sh all --package                everything this host can build, with a summary
  ./build.sh pack                         source .tgz in /tmp/
  ./build.sh bench-gate --only jpeg,png   the speed gate for the JPEG and PNG cases
  ./build.sh bench-gate --suite app --bless  re-bless the app gate
EOF
}

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
MODE=release
PLATFORM=host
PACKAGE=0
PGO=0
SIGN=0
OUT=
SLICE=
FROM=

# --- output style -------------------------------------------------------------------------------
# Colour on a terminal unless NO_COLOR (https://no-color.org) or NO_COLORS is set, or --no-color(s)
# is given;
# FORCE_COLOR / CLICOLOR_FORCE keep it for CI logs. Symbols fall back to ASCII outside UTF-8.
START=$SECONDS
color=0
if [[ -z "${NO_COLOR:-}${NO_COLORS:-}" ]]; then
    if [[ -n "${FORCE_COLOR:-}${CLICOLOR_FORCE:-}" ]] || { [[ -t 1 ]] && [[ "${TERM:-}" != dumb ]]; }; then
        color=1
    fi
fi
for a in "$@"; do [[ $a == --no-color || $a == --no-colors ]] && color=0; done
if [[ $color == 1 ]]; then
    C_RESET=$'\033[0m' C_BOLD=$'\033[1m' C_DIM=$'\033[2m' C_RED=$'\033[31m' C_GREEN=$'\033[32m'
    C_YELLOW=$'\033[33m' C_BLUE=$'\033[34m' C_MAGENTA=$'\033[35m' C_CYAN=$'\033[36m'
    # cargo colours its own output only on a terminal; follow our choice when forced.
    [[ -t 2 ]] || export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
else
    # Asked for plain output: cargo and the tools it runs too.
    if [[ -n "${NO_COLOR:-}${NO_COLORS:-}" || " $* " == *' --no-color '* || " $* " == *' --no-colors '* ]]; then
        export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-never}" NO_COLOR="${NO_COLOR:-1}"
    fi
    C_RESET='' C_BOLD='' C_DIM='' C_RED='' C_GREEN='' C_YELLOW='' C_BLUE='' C_MAGENTA='' C_CYAN=''
fi
case "${LC_ALL:-${LC_CTYPE:-${LANG:-}}}" in
    *[Uu][Tt][Ff]-8*|*[Uu][Tt][Ff]8*)
        S_OK='✔' S_FAIL='✖' S_SKIP='⊘' S_WARN='▲' S_DOT='·' S_RULE='━' S_LOGO='◆' ;;
    *)
        S_OK='ok' S_FAIL='x' S_SKIP='-' S_WARN='!' S_DOT='-' S_RULE='-' S_LOGO='*' ;;
esac
COLS=${COLUMNS:-$( (tput cols 2>/dev/null) || echo 80)}
[[ $COLS =~ ^[0-9]+$ ]] || COLS=80
(( COLS > 100 )) && COLS=100
(( COLS < 40 )) && COLS=40

elapsed() { # elapsed SECONDS -> "42s" or "3m 07s"
    local s=$1
    if (( s < 60 )); then printf '%ds' "$s"; else printf '%dm %02ds' $((s / 60)) $((s % 60)); fi
}
die() { printf '\n%s%s error:%s %s\n' "$C_RED$C_BOLD" "$S_FAIL" "$C_RESET" "$*" >&2; exit 1; }
warn() { printf '  %s%s warning:%s %s\n' "$C_YELLOW$C_BOLD" "$S_WARN" "$C_RESET" "$*" >&2; }
# --- pack: the code and docs as a .tgz ----------------------------------------------------------
pack() {
    cd "$ROOT"
    if ! command -v git >/dev/null || ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        die "pack needs git and a git checkout (it packs the files git tracks or would track)"
    fi
    local version prefix out list n
    version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
    prefix="specular-${version:-0}-src-$(date +%Y%m%d)"
    out=${1:-/tmp/$prefix.tgz}
    case "$out" in /*) ;; *) out="$PWD/$out" ;; esac
    mkdir -p "$ROOT/target/tmp"
    list=$(mktemp "$ROOT/target/tmp/specular-pack.XXXXXX")
    # Tracked and untracked-but-not-ignored files that exist (not ones deleted in the working
    # tree). The private photo corpus is excluded explicitly too, should .gitignore ever change.
    git ls-files -z --cached --others --exclude-standard | while IFS= read -r -d '' f; do
        case "$f" in tools/corpus/real/*) continue ;; esac
        [[ -f $f || -L $f ]] && printf '%s\0' "$f"
    done >"$list"
    n=$(tr -cd '\0' <"$list" | wc -c | tr -d ' ')
    if tar --version 2>/dev/null | grep -q 'GNU tar'; then
        tar --null -T "$list" --transform "s,^,$prefix/," -czf "$out"
    else
        tar --null -T "$list" -s ",^,$prefix/," -czf "$out"
    fi
    rm -f "$list"
    printf '%s%s%s %s files %s %s (%s)\n' "$C_GREEN" "$S_OK" "$C_RESET" "$n" "$S_DOT" "$out" \
        "$(du -h "$out" | cut -f1)"
    local dirty
    dirty=$(git status --porcelain | wc -l | tr -d ' ')
    [[ $dirty == 0 ]] || warn "$dirty uncommitted changes are included (the working tree is packed, not HEAD)"
}
if [[ ${1:-} == pack ]]; then
    [[ $# -le 2 ]] || die "pack takes at most one argument, the output file"
    pack "${2:-}"
    exit 0
fi
if [[ ${1:-} == bench-gate ]]; then
    shift
    if [[ " $* " == *" --suite develop "* ]]; then
        # Develop's GPU timing gate: its own bench, not perf-gate.py.
        export DV_BENCH_GATE=1
        [[ " $* " == *" --bless "* ]] && export SPECULAR_BLESS=1
        exec cargo bench -p specular-develop --profile dev --bench develop
    fi
    if [[ " $* " != *" --suite app "* ]]; then
        command -v valgrind >/dev/null || die "bench-gate needs valgrind (e.g. apt install valgrind)"
    fi
    exec python3 -I "$ROOT/tools/bench/perf-gate.py" "$@"
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        release|debug) MODE=$1 ;;
        linux|windows|macos|host|all) PLATFORM=$1 ;;
        --package) PACKAGE=1 ;;
        --pgo) PGO=1 ;;
        --sign) SIGN=1; PACKAGE=1 ;;
        --out)
            [[ $# -ge 2 ]] || die "--out needs a directory"
            case "$2" in /*) OUT=$2 ;; *) OUT="$PWD/$2" ;; esac
            shift ;;
        --slice)
            [[ $# -ge 2 ]] || die "--slice needs a target"
            SLICE=$2
            shift ;;
        --from)
            [[ $# -ge 2 ]] || die "--from needs a directory"
            case "$2" in /*) FROM=$2 ;; *) FROM="$PWD/$2" ;; esac
            shift ;;
        --no-color|--no-colors) ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; die "unknown argument '$1'" ;;
    esac
    shift
done
[[ $PGO == 1 && $MODE == debug ]] && die "--pgo needs a release build (drop 'debug' or '--pgo')"
cd "$ROOT"

# Some archivers drop the executable bits (a source .tgz extracted without them, a zip): give
# them back to every script (a file starting with "#!/") so the packaging steps can run them.
fix_exec_bits() {
    local f h fixed=()
    while IFS= read -r -d '' f; do
        h=
        IFS= read -r -n 3 h <"$f" 2>/dev/null || true
        [[ $h == '#!/' ]] && chmod +x "$f" && fixed+=("$f")
    done < <(find . \( -name target -o -name .git \) -prune -o -type f ! -perm -u+x \
        \( -name '*.sh' -o -name '*.bash' -o -name '*.py' -o -name '*.pl' -o ! -name '*.*' \) -print0)
    if (( ${#fixed[@]} > 0 )); then
        printf '  %s%s made %d scripts executable (the archive had dropped their x bits)%s\n' \
            "$C_DIM" "$S_DOT" "${#fixed[@]}" "$C_RESET"
    fi
}
fix_exec_bits

# --- environment --------------------------------------------------------------------------------
case "$(uname -s)" in
    Linux) HOST=linux ;;
    Darwin) HOST=macos ;;
    MINGW*|MSYS*|CYGWIN*) HOST=windows ;;
    *) die "unsupported host $(uname -s)" ;;
esac
[[ $PLATFORM == host ]] && PLATFORM=$HOST
if [[ -n $SLICE$FROM ]]; then
    [[ $PLATFORM == macos ]] || die "--slice and --from are for macos builds"
    [[ -n $SLICE && $PACKAGE == 1 ]] && die "--slice builds half a binary, which is not packaged: drop --package/--sign"
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
case "$CARGO_TARGET_DIR" in /*|[A-Za-z]:*) ;; *) CARGO_TARGET_DIR="$ROOT/$CARGO_TARGET_DIR" ;; esac
case "${TMPDIR:-}" in
    "$ROOT/target"/*) ;;
    *) TMPDIR="$ROOT/target/tmp" ;;
esac
export TMPDIR
TOOLS="$ROOT/target/tools"
export PATH="$TOOLS/bin:$PATH"
export XWIN_CACHE_DIR="${XWIN_CACHE_DIR:-$TOOLS/xwin}"   # cargo-xwin defaults to ~/.cache
mkdir -p "$TMPDIR" "$TOOLS"

ALL=0
[[ $PLATFORM == all ]] && ALL=1
SUFFIX=
PROFILE_DIR=release
# Release builds include rav1d's and rav1e's hand-written assembly for AVIF decode and output
# (feature specular/asm), which needs nasm on x86_64 (aarch64 assembles with the C compiler).
CARGO_PROFILE=(--release --features specular/asm)
if [[ $MODE == debug ]]; then
    SUFFIX=-debug
    PROFILE_DIR=debug
    CARGO_PROFILE=()
fi

platform_color() {
    case "$1" in
        linux) printf '%s' "$C_YELLOW" ;;
        windows) printf '%s' "$C_BLUE" ;;
        macos) printf '%s' "$C_MAGENTA" ;;
        *) printf '%s' "$C_CYAN" ;;
    esac
}
# step "linux: cargo build (release)": a full-width rule with the platform in its colour and the
# time since the build started on the right.
step() {
    local text=$* plat='' rest pc t pad n
    if [[ $text == *': '* ]]; then plat=${text%%: *}; rest=${text#*: }; else rest=$text; fi
    t=$(elapsed $((SECONDS - START)))
    if [[ -n $plat ]]; then
        pc=$(platform_color "$plat")
        n=$((COLS - ${#plat} - ${#rest} - ${#t} - 10))
    else
        n=$((COLS - ${#rest} - ${#t} - 7))
    fi
    (( n < 3 )) && n=3
    printf -v pad '%*s' "$n" ''
    pad=${pad// /$S_RULE}
    if [[ -n $plat ]]; then
        printf '\n%s%s%s %s%s%s %s %s%s%s %s%s %s%s\n' "$C_DIM" "$S_RULE$S_RULE" "$C_RESET" \
            "$C_BOLD$pc" "$plat" "$C_RESET" "$S_DOT" "$C_BOLD" "$rest" "$C_RESET" \
            "$C_DIM" "$pad" "$t" "$C_RESET"
    else
        printf '\n%s%s%s %s%s%s %s%s %s%s\n' "$C_DIM" "$S_RULE$S_RULE" "$C_RESET" \
            "$C_BOLD" "$rest" "$C_RESET" "$C_DIM" "$pad" "$t" "$C_RESET"
    fi
}
note() { printf '  %s%s %s%s\n' "$C_DIM" "$S_DOT" "$*" "$C_RESET"; }
ok() { printf '  %s%s%s %s\n' "$C_GREEN$C_BOLD" "$S_OK" "$C_RESET" "$*"; }
fail() { printf '  %s%s%s %s\n' "$C_RED$C_BOLD" "$S_FAIL" "$C_RESET" "$*" >&2; }
skipped() { printf '  %s%s skipped:%s %s\n' "$C_YELLOW" "$S_SKIP" "$C_RESET" "$*"; }
# The path shown to people: relative to the repository when inside it.
rel() { printf '%s' "${1#"$ROOT/"}"; }
have() { command -v "$1" >/dev/null 2>&1; }
# need TOOL HOW-TO-INSTALL: notes TOOL as missing; need_check then reports every missing tool at
# once, so one run lists all there is to install rather than one per run.
MISSING=()
need() {
    have "$1" || MISSING+=("$1|$2")
}
need_check() {
    (( ${#MISSING[@]} > 0 )) || return 0
    local m w=0
    for m in "${MISSING[@]}"; do
        m=${m%%|*}
        (( ${#m} > w )) && w=${#m}
    done
    printf '\n%s%s error:%s %d required tools not found:\n' "$C_RED$C_BOLD" "$S_FAIL" "$C_RESET" \
        "${#MISSING[@]}" >&2
    for m in "${MISSING[@]}"; do
        printf '    %s%-*s%s  %s%s%s\n' "$C_BOLD" "$w" "${m%%|*}" "$C_RESET" "$C_DIM" "${m#*|}" "$C_RESET" >&2
    done
    exit 1
}
sha256_file() { # writes FILE.sha256 next to FILE
    local d f
    d=$(dirname "$1"); f=$(basename "$1")
    if have sha256sum; then
        (cd "$d" && sha256sum "$f" >"$f.sha256")
    else
        (cd "$d" && shasum -a 256 "$f" >"$f.sha256")
    fi
}
sha256_of() { cut -d' ' -f1 <"$1.sha256"; }
size_of() { du -h "$1" | cut -f1; }

# Windows: tools that installers leave off PATH (NASM, LLVM) or that come with Visual Studio
# (Ninja, its own clang-cl) are looked for in their usual folders and appended to PATH.
VS_DIR=
if [[ $HOST == windows ]]; then
    # on_path TOOL DIR...: unless TOOL is on PATH, appends the first DIR holding TOOL.exe.
    on_path() {
        local tool=$1 d
        shift
        have "$tool" && return 0
        for d in "$@"; do
            [[ -x "$d/$tool.exe" ]] && { PATH="$PATH:$d"; return 0; }
        done
        return 1
    }
    vswhere="$(cygpath -u "$(printenv 'ProgramFiles(x86)' || echo 'C:/Program Files (x86)')")/Microsoft Visual Studio/Installer/vswhere.exe"
    if [[ -x $vswhere ]]; then
        # The newest Visual Studio (or Build Tools) with the C++ x64 compiler and linker.
        VS_DIR=$("$vswhere" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 \
            -property installationPath | tr -d '\r' || true)
        [[ -n $VS_DIR ]] && VS_DIR=$(cygpath -u "$VS_DIR")
    fi
    pf=$(cygpath -u "${PROGRAMFILES:-C:/Program Files}")
    on_path nasm "$pf/NASM" ${LOCALAPPDATA:+"$(cygpath -u "$LOCALAPPDATA")/bin/NASM"} || true
    on_path clang-cl "$pf/LLVM/bin" ${VS_DIR:+"$VS_DIR/VC/Tools/Llvm/x64/bin"} || true
    on_path ninja ${VS_DIR:+"$VS_DIR/Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja"} || true
fi

need cargo "Rust via https://rustup.rs"
need cmake "e.g. apt install cmake, dnf install cmake, brew install cmake, winget install Kitware.CMake"
[[ $MODE == release ]] && need nasm "e.g. apt install nasm, dnf install nasm, brew install nasm, winget install NASM.NASM"
# The Linux system libraries the crates link (fonts, keyboard, Wayland, sound), through pkg-config.
if [[ $HOST == linux && ( $PLATFORM == linux || $PLATFORM == all ) ]]; then
    need pkg-config "e.g. apt install pkg-config, dnf install pkgconf"
    if have pkg-config; then
        for lib in fontconfig:libfontconfig1-dev:fontconfig-devel xkbcommon:libxkbcommon-dev:libxkbcommon-devel \
            wayland-client:libwayland-dev:wayland-devel alsa:libasound2-dev:alsa-lib-devel; do
            IFS=: read -r pc deb rpm <<<"$lib"
            pkg-config --exists "$pc" || MISSING+=("$pc|e.g. apt install $deb, dnf install $rpm")
        done
    fi
fi
if [[ $HOST == macos && ( $PLATFORM == macos || $PLATFORM == all ) ]]; then
    need rustup "https://rustup.rs (needed to add the aarch64/x86_64 Apple targets)"
fi
need_check
# rustup 1.28+ no longer installs the toolchain pinned in rust-toolchain.toml on first use.
if have rustup; then
    TOOLCHAIN=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
    if ! rustup toolchain list | grep -q "^$TOOLCHAIN"; then
        step "Installing the pinned Rust toolchain $TOOLCHAIN"
        rustup toolchain install "$TOOLCHAIN" --profile minimal \
            --component rustfmt --component clippy --component rust-src \
            || die "could not install Rust toolchain $TOOLCHAIN"
    fi
    # Every cargo here and in the packaging scripts runs the pinned toolchain, as
    # `rustup run $TOOLCHAIN cargo` would: its bin folder goes first on PATH (ahead of a cargo
    # from Homebrew or a distro) and RUSTUP_TOOLCHAIN beats a directory override or a default.
    TC_SYSROOT=$(rustup run "$TOOLCHAIN" rustc --print sysroot) \
        || die "rustup could not run Rust toolchain $TOOLCHAIN"
    [[ $HOST == windows ]] && TC_SYSROOT=$(cygpath -u "$TC_SYSROOT")
    export RUSTUP_TOOLCHAIN="$TOOLCHAIN"
    export PATH="$TC_SYSROOT/bin:$PATH"
    note "Rust toolchain $TOOLCHAIN ($(cargo --version))"
fi
# cargo's own error (toolchain not installed, unreadable manifest, ...) is shown if this fails.
META_ERR=$(mktemp "${TMPDIR:-/tmp}/specular-meta.XXXXXX")
VERSION=$(cargo metadata --no-deps --format-version 1 --offline 2>"$META_ERR" \
    | grep -o '"name":"specular","version":"[^"]*"' | head -1 | sed 's/.*"version":"\(.*\)"/\1/' || true)
if [[ -z "$VERSION" ]]; then
    sed 's/^/    /' "$META_ERR" >&2
    rm -f "$META_ERR"
    die "could not read the specular version from 'cargo metadata' (cargo's output is above; run it in $ROOT to retry)"
fi
rm -f "$META_ERR"
DIST="${OUT:-$ROOT/target/dist/$VERSION}"

# Summary rows: "platform|artifact|status".
RESULTS=()
FAILED=0
result() { RESULTS+=("$1|$2|$3"); }
# A missing prerequisite stops a single-platform build with an error; with 'all' the platform (or
# package) is skipped and the reason goes into the summary. Returns 1 when skipped.
missing() { # missing PLATFORM ARTIFACT MESSAGE
    if [[ $ALL == 1 ]]; then
        result "$1" "$2" "skipped: $3"
        skipped "$3"
        return 1
    fi
    die "$3"
}
# Runs a packaging step; a failure is recorded and the remaining steps still run.
run_pkg() { # run_pkg PLATFORM ARTIFACT-PATH COMMAND...
    local plat=$1 art=$2
    shift 2
    # Not `if "$@"`: bash ignores `set -e` inside anything run as a condition.
    local rc=0
    set +e
    (set -e; "$@")
    rc=$?
    set -e
    if [[ $rc != 0 ]]; then
        result "$plat" "$art" "FAILED: exit $rc"
        fail "$(rel "$art"): failed (exit $rc)"
        FAILED=1
    elif [[ -e "$art" ]]; then
        result "$plat" "$art" "built ($(size_of "$art"))"
        ok "$(rel "$art") $C_DIM($(size_of "$art"))$C_RESET"
    else
        result "$plat" "$art" "FAILED: not produced"
        fail "$(rel "$art"): the step succeeded but produced nothing"
        FAILED=1
    fi
}

# Banner: what is being built and where it goes.
extras=
[[ $PACKAGE == 1 ]] && extras+=" $S_DOT packages"
[[ $PGO == 1 ]] && extras+=" $S_DOT PGO"
[[ $SIGN == 1 ]] && extras+=" $S_DOT signed"
printf '\n  %s%s Specular%s %s%s%s\n' "$C_CYAN$C_BOLD" "$S_LOGO" "$C_RESET" "$C_BOLD" "$VERSION" "$C_RESET"
printf '    %s%s%s build for %s%s%s%s\n' "$C_BOLD" "$MODE" "$C_RESET" \
    "$C_BOLD$(platform_color "$PLATFORM")" "$PLATFORM" "$C_RESET" "$extras"
printf '    %soutput%s  %s/<platform>/\n' "$C_DIM" "$C_RESET" "$(rel "$DIST")"
printf '    %scargo%s   %s\n' "$C_DIM" "$C_RESET" "$(rel "$CARGO_TARGET_DIR")"

# --- Linux --------------------------------------------------------------------------------------
build_linux() {
    local dir="$DIST/linux" bin host pgo_out bolt_dir
    if [[ $HOST != linux ]]; then
        missing linux - "Linux builds need a Linux host" || return 0
    fi
    mkdir -p "$dir"
    if [[ $PGO == 1 ]]; then
        step "linux: PGO build (tools/release/pgo.sh)"
        pgo_out="$CARGO_TARGET_DIR/pgo"
        local args=(--out "$pgo_out")
        bolt_dir=
        if have llvm-bolt; then
            bolt_dir=$(dirname "$(dirname "$(command -v llvm-bolt)")")
        elif [[ -x "$TOOLS/llvm-bolt/bin/llvm-bolt" ]]; then
            bolt_dir="$TOOLS/llvm-bolt"
        fi
        if [[ -n "$bolt_dir" ]]; then
            args+=(--bolt)
        else
            note "llvm-bolt not found (PATH or target/tools/llvm-bolt): PGO without BOLT"
        fi
        LLVM_BOLT_DIR="$bolt_dir" tools/release/pgo.sh "${args[@]}"
        bin="$pgo_out/dist/specular"
        [[ -x "$pgo_out/dist/specular.bolt" ]] && bin="$pgo_out/dist/specular.bolt"
        host="$pgo_out/dist/specular-decode-host"
    else
        step "linux: cargo build ($MODE)"
        cargo build --locked ${CARGO_PROFILE[@]+"${CARGO_PROFILE[@]}"} -p specular --bin specular \
            -p specular-decode-host --bin specular-decode-host
        bin="$CARGO_TARGET_DIR/$PROFILE_DIR/specular"
        host="$CARGO_TARGET_DIR/$PROFILE_DIR/specular-decode-host"
    fi
    # The sandboxed decode host (HEIC, RAR/CBR) must sit next to the viewer.
    cp "$bin" "$dir/specular$SUFFIX"
    cp "$host" "$dir/specular-decode-host"
    result linux "$dir/specular$SUFFIX" "built ($(size_of "$dir/specular$SUFFIX"))"
    result linux "$dir/specular-decode-host" "built ($(size_of "$dir/specular-decode-host"))"
    ok "$(rel "$dir/specular$SUFFIX") $C_DIM($(size_of "$dir/specular$SUFFIX"))$C_RESET"
    ok "$(rel "$dir/specular-decode-host") $C_DIM($(size_of "$dir/specular-decode-host"))$C_RESET"
    [[ $PACKAGE == 1 ]] || return 0

    # tar.gz: binary (release: stripped, line tables in a separate .debug file), licences,
    # desktop integration files for a manual install into ~/.local or /usr/local.
    step "linux: tar.gz"
    local name="specular-$VERSION-linux-x86_64$SUFFIX" stage="$ROOT/target/package/linux"
    local tgz="$dir/$name.tar.gz"
    pkg_tar() {
        local s="$stage/$name" objcopy
        rm -rf "$stage"; mkdir -p "$s/share/doc/specular"
        install -m755 "$bin" "$s/specular"
        install -m755 "$host" "$s/specular-decode-host"
        if [[ $MODE == release ]]; then
            objcopy=$(command -v llvm-objcopy || command -v objcopy) \
                || { echo "objcopy not found: install binutils" >&2; return 1; }
            local b
            for b in specular specular-decode-host; do
                "$objcopy" --only-keep-debug "$s/$b" "$dir/$b-$VERSION-x86_64$SUFFIX.debug"
                "$objcopy" --strip-all --add-gnu-debuglink="$dir/$b-$VERSION-x86_64$SUFFIX.debug" "$s/$b"
            done
        fi
        install -m644 packaging/THIRD-PARTY.md "$s/share/doc/specular/"
        install -m644 LICENSE-MIT LICENSE-APACHE "$s/share/doc/specular/"
        install -Dm644 packaging/linux/app.specular.Specular.desktop "$s/share/applications/app.specular.Specular.desktop"
        install -Dm644 packaging/linux/app.specular.Specular.metainfo.xml "$s/share/metainfo/app.specular.Specular.metainfo.xml"
        install -Dm644 packaging/linux/app.specular.Specular.mime.xml "$s/share/mime/packages/app.specular.Specular.xml"
        install -Dm644 packaging/icons/specular.svg "$s/share/icons/hicolor/scalable/apps/app.specular.Specular.svg"
        install -Dm644 packaging/linux/specular.thumbnailer "$s/share/thumbnailers/specular.thumbnailer"
        tar -C "$stage" -czf "$tgz" "$name"
        sha256_file "$tgz"
    }
    run_pkg linux "$tgz" pkg_tar

    step "linux: AppImage"
    local appimage_args=(--bin "$bin" --host "$host" --out "$dir" --suffix "$SUFFIX")
    [[ $MODE == debug ]] && appimage_args+=(--no-split)
    run_pkg linux "$dir/Specular-$VERSION-x86_64$SUFFIX.AppImage" \
        packaging/linux/build-appimage.sh "${appimage_args[@]}"

    step "linux: .deb and .rpm"
    local split_args=()
    [[ $MODE == debug ]] && split_args+=(--no-split)
    local rver=${VERSION//-/_}
    if have dpkg-deb && { [[ $(id -u) == 0 ]] || have fakeroot; }; then
        run_pkg linux "$dir/specular_${VERSION}${SUFFIX}_amd64.deb" packaging/linux/build-deb-rpm.sh \
            --deb-only --bin "$bin" --host "$host" --out "$dir" --suffix "$SUFFIX" ${split_args[@]+"${split_args[@]}"}
    else
        missing linux "$dir/specular_${VERSION}${SUFFIX}_amd64.deb" "dpkg-deb/fakeroot not found. Install them: sudo apt install dpkg-dev fakeroot" || true
    fi
    if have rpmbuild; then
        run_pkg linux "$dir/specular-$rver-1.x86_64$SUFFIX.rpm" packaging/linux/build-deb-rpm.sh \
            --rpm-only --bin "$bin" --host "$host" --out "$dir" --suffix "$SUFFIX" ${split_args[@]+"${split_args[@]}"}
    else
        missing linux "$dir/specular-$rver-1.x86_64$SUFFIX.rpm" "rpmbuild not found. Install it: sudo apt install rpm (or dnf install rpm-build)" || true
    fi

    step "linux: Flatpak"
    local fp="$dir/Specular-$VERSION-x86_64$SUFFIX.flatpak"
    if ! have flatpak-builder || ! have flatpak; then
        missing linux "$fp" "flatpak-builder not found. Install it: sudo apt install flatpak flatpak-builder (or dnf install flatpak-builder)" || return 0
    fi
    if ! flatpak info org.freedesktop.Platform//25.08 >/dev/null 2>&1 \
        || ! flatpak info org.freedesktop.Sdk//25.08 >/dev/null 2>&1; then
        missing linux "$fp" "Flatpak runtime missing. Install it: flatpak install flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08" || return 0
    fi
    run_pkg linux "$fp" packaging/linux/build-flatpak.sh --local --bin "$bin" --host "$host" --out "$dir" --suffix "$SUFFIX"
}

# --- Windows ------------------------------------------------------------------------------------
build_windows() {
    local dir="$DIST/windows" triple=x86_64-pc-windows-msvc exe cross=0
    [[ $PGO == 1 ]] && warn "--pgo is Linux-only for now; building Windows without PGO"
    if [[ $HOST != windows ]]; then
        cross=1
        if ! have cargo-xwin; then
            missing windows - "cargo-xwin not found (needed to cross-compile for Windows). Install it: cargo install --locked --root target/tools cargo-xwin" || return 0
        fi
        if ! rustup target list --installed 2>/dev/null | grep -qx "$triple"; then
            missing windows - "Rust target $triple not installed. Install it: rustup target add $triple" || return 0
        fi
        if ! have clang-cl || ! have lld-link; then
            missing windows - "clang-cl/lld-link not found (cargo-xwin needs LLVM). Install them: sudo apt install clang lld llvm" || return 0
        fi
        if [[ "${SPECULAR_ACCEPT_MSVC_LICENSE:-}" != 1 ]]; then
            missing windows - "cross-compiling for Windows downloads the MSVC CRT and Windows SDK, which are under Microsoft's licence (https://go.microsoft.com/fwlink/?LinkId=2086102). If you accept it, rerun with SPECULAR_ACCEPT_MSVC_LICENSE=1 (files are cached in target/tools/xwin)" || return 0
        fi
        if [[ $SIGN == 1 ]]; then
            missing windows - "--sign for Windows needs signtool on a Windows host; cross builds can't be signed here" || return 0
        fi
    elif [[ $SIGN == 1 ]]; then
        need signtool "the Windows SDK (signtool.exe on PATH)"
        need_check
        [[ -n "${SPECULAR_SIGN_CERT_SHA1:-}" ]] || die "--sign on Windows needs SPECULAR_SIGN_CERT_SHA1 (thumbprint of the code-signing certificate)"
    fi
    if [[ $cross == 0 ]]; then
        if [[ -z $VS_DIR ]]; then
            missing windows - "Visual Studio 2022 (or its Build Tools) with the \"Desktop development with C++\" workload not found: it provides the MSVC linker and the Windows SDK" || return 0
        fi
        # libjxl (jpegxl-src) is compiled by clang-cl: run by Ninja directly when both are
        # there, otherwise through Visual Studio's optional ClangCL MSBuild toolset
        # (third_party/jpegxl-src/patches). A CMAKE_GENERATOR set by the caller is kept.
        local toolset=0 d
        for d in "$VS_DIR"/MSBuild/Microsoft/VC/v*/Platforms/x64/PlatformToolsets/ClangCL; do
            [[ -d $d ]] && toolset=1
        done
        if [[ -z ${CMAKE_GENERATOR:-} ]] && have ninja && have clang-cl; then
            export CMAKE_GENERATOR=Ninja
        fi
        if [[ ${CMAKE_GENERATOR:-} == *Ninja* ]]; then
            if ! have ninja || ! have clang-cl; then
                missing windows - "CMAKE_GENERATOR=$CMAKE_GENERATOR needs ninja and clang-cl. Install them: winget install Ninja-build.Ninja; winget install LLVM.LLVM" || return 0
            fi
            note "libjxl: Ninja with $(command -v clang-cl)"
        elif [[ $toolset == 1 ]]; then
            note "libjxl: Visual Studio's ClangCL toolset"
        else
            missing windows - "libjxl needs clang-cl, run by Ninja or by Visual Studio's ClangCL toolset. Install LLVM and Ninja (winget install LLVM.LLVM; winget install Ninja-build.Ninja), or add Visual Studio's \"MSBuild support for LLVM (clang-cl) toolset\" component" || return 0
        fi
    fi
    mkdir -p "$dir"
    if [[ $cross == 1 ]]; then
        step "windows: cargo xwin build ($MODE, cross from $HOST)"
        cargo xwin build --locked ${CARGO_PROFILE[@]+"${CARGO_PROFILE[@]}"} --target "$triple" -p specular --bin specular \
            -p specular-decode-host --bin specular-decode-host
    else
        step "windows: cargo build ($MODE)"
        cargo build --locked ${CARGO_PROFILE[@]+"${CARGO_PROFILE[@]}"} --target "$triple" -p specular --bin specular \
            -p specular-decode-host --bin specular-decode-host
    fi
    local host_exe="$CARGO_TARGET_DIR/$triple/$PROFILE_DIR/specular-decode-host.exe"
    exe="$CARGO_TARGET_DIR/$triple/$PROFILE_DIR/specular.exe"
    local pdb="$CARGO_TARGET_DIR/$triple/$PROFILE_DIR/specular.pdb"
    if [[ $SIGN == 1 ]]; then
        step "windows: sign specular.exe"
        signtool sign /sha1 "$SPECULAR_SIGN_CERT_SHA1" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$exe" "$host_exe"
    fi
    cp "$exe" "$dir/specular$SUFFIX.exe"
    cp "$host_exe" "$dir/specular-decode-host.exe"
    [[ -f "$pdb" ]] && cp "$pdb" "$dir/specular$SUFFIX.pdb"
    result windows "$dir/specular$SUFFIX.exe" "built ($(size_of "$dir/specular$SUFFIX.exe"))"
    ok "$(rel "$dir/specular$SUFFIX.exe") $C_DIM($(size_of "$dir/specular$SUFFIX.exe"))$C_RESET"
    [[ $PACKAGE == 1 ]] || return 0

    step "windows: zip"
    local name="Specular-$VERSION-windows-x64$SUFFIX" zipf="$dir/Specular-$VERSION-windows-x64$SUFFIX.zip"
    pkg_zip() {
        local stage="$ROOT/target/package/windows/$name"
        rm -rf "$stage"; mkdir -p "$stage"
        cp "$exe" "$stage/specular.exe"
        # Next to the viewer, which starts it in an AppContainer from wherever the copy was
        # unzipped (no folder permission needed; see specular-sandbox).
        cp "$host_exe" "$stage/specular-decode-host.exe"
        [[ $MODE == debug && -f "$pdb" ]] && cp "$pdb" "$stage/specular.pdb"
        # A portable copy: the marker keeps settings and caches in the folder (specular-settings
        # paths.rs; the same text the viewer writes when portable mode is turned on).
        printf 'Specular portable mode: settings are kept in this folder.\n' >"$stage/portable"
        cp packaging/windows/README-portable.txt "$stage/README.txt"
        cp packaging/THIRD-PARTY.md "$stage/"
        cp LICENSE-MIT LICENSE-APACHE "$stage/"
        rm -f "$zipf"
        if have zip; then
            (cd "$(dirname "$stage")" && zip -qr9 "$zipf" "$name")
        elif have 7z; then
            (cd "$(dirname "$stage")" && 7z a -tzip -bso0 "$zipf" "$name")
        elif [[ $HOST == windows && -x "$(cygpath -u "${SYSTEMROOT:-C:/Windows}")/System32/tar.exe" ]]; then
            # Windows 10+'s bsdtar. Ahead of Compress-Archive, whose Windows PowerShell 5.1 writes
            # entry names with backslashes, which unzip elsewhere keeps as part of the file name.
            (cd "$(dirname "$stage")" && "$(cygpath -u "${SYSTEMROOT:-C:/Windows}")/System32/tar.exe" \
                -a -c -f "$(cygpath -w "$zipf")" "$name")
        elif have powershell.exe; then
            powershell.exe -NoProfile -Command "Compress-Archive -Path '$(cygpath -w "$stage")' -DestinationPath '$(cygpath -w "$zipf")'"
        else
            echo "no zip tool: install zip (apt install zip / pacman -S zip)" >&2; return 1
        fi
        sha256_file "$zipf"
    }
    run_pkg windows "$zipf" pkg_zip

    step "windows: MSI (WiX v4)"
    local msi="$dir/Specular-$VERSION-x64$SUFFIX.msi"
    if ! have wix; then
        if [[ $HOST == windows ]]; then
            missing windows "$msi" "wix not found. Install it: dotnet tool install -g wix --version '4.*'" || return 0
        fi
        result windows "$msi" "skipped: MSI needs WiX v4 (Windows; 'dotnet tool install -g wix --version 4.*')"
        skipped "MSI: WiX is not available on this $HOST host"
        result windows "$dir/winget/" "skipped: needs the MSI"
        return 0
    fi
    pkg_msi() {
        local bindir pkgdir out ui
        bindir=$(dirname "$exe"); pkgdir="$ROOT/packaging"; out=$msi
        if have cygpath; then
            bindir=$(cygpath -w "$bindir"); pkgdir=$(cygpath -w "$pkgdir"); out=$(cygpath -w "$msi")
        fi
        # The UI extension pinned to wix's own version ("4.0.6+hash" -> 4.0.6), when adding and
        # when building: unpinned, `extension add` fetches the newest one and `-ext` picks the
        # newest cached, which an older wix cannot load (WIX0144).
        ui=$(wix --version | tr -d '\r')
        ui="WixToolset.UI.wixext/${ui%%+*}"
        wix extension add -g "$ui" >/dev/null
        wix build packaging/windows/specular.wxs -arch x64 -ext "$ui" \
            -d "Version=$VERSION" -d "BinDir=$bindir" -d "PackagingDir=$pkgdir" -o "$out"
        if [[ $SIGN == 1 ]]; then
            signtool sign /sha1 "$SPECULAR_SIGN_CERT_SHA1" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "$msi"
        fi
        sha256_file "$msi"
    }
    run_pkg windows "$msi" pkg_msi
    [[ -f "$msi" ]] || { result windows "$dir/winget/" "skipped: needs the MSI"; return 0; }

    # winget manifests (release only), filled in from packaging/windows/winget with the MSI hash.
    if [[ $MODE == debug ]]; then
        result windows "$dir/winget/" "skipped: winget manifests are for release builds"
        return 0
    fi
    step "windows: winget manifests"
    local wdir="$dir/winget/manifests/s/Specular/Specular/$VERSION"
    pkg_winget() {
        local sha url product_code=
        sha=$(sha256_of "$msi" | tr '[:lower:]' '[:upper:]')
        url="${SPECULAR_RELEASE_URL:-https://github.com/amazynk/specular/releases/download/v$VERSION}/$(basename "$msi")"
        if have msiinfo; then
            product_code=$(msiinfo export "$msi" Property | awk -F'\t' '$1=="ProductCode"{print $2}' | tr -d '\r')
        elif have powershell.exe; then
            product_code=$(powershell.exe -NoProfile -Command "\$i=New-Object -ComObject WindowsInstaller.Installer; \$d=\$i.GetType().InvokeMember('OpenDatabase','InvokeMethod',\$null,\$i,@('$(cygpath -w "$msi")',0)); \$v=\$d.OpenView(\"SELECT Value FROM Property WHERE Property='ProductCode'\"); \$v.Execute(); \$v.Fetch().StringData(1)" | tr -d '\r')
        fi
        rm -rf "$dir/winget"; mkdir -p "$wdir"
        local f
        for f in packaging/windows/winget/*.yaml; do
            sed -e "s|{{VERSION}}|$VERSION|g" -e "s|{{MSI_URL}}|$url|g" -e "s|{{MSI_SHA256}}|$sha|g" \
                -e "s|{{PRODUCT_CODE}}|$product_code|g" "$f" >"$wdir/$(basename "$f")"
        done
        if [[ -z "$product_code" ]]; then
            note "ProductCode not readable here (needs msiinfo or Windows): line removed (it is optional)"
            sed -i.bak '/ProductCode:/d' "$wdir/Specular.Specular.installer.yaml" && rm -f "$wdir"/*.bak
        fi
        [[ -n "${SPECULAR_RELEASE_URL:-}" ]] || note "InstallerUrl uses a placeholder: set SPECULAR_RELEASE_URL"
    }
    run_pkg windows "$wdir/Specular.Specular.installer.yaml" pkg_winget
}

# --- macOS --------------------------------------------------------------------------------------
build_macos() {
    local dir="$DIST/macos"
    if [[ $HOST != macos ]]; then
        # Not an error, even for an explicit 'macos': there is nothing to do without a Mac.
        result macos - "skipped: macOS builds need a Mac (Xcode tools: lipo, codesign, hdiutil)"
        skipped "macOS packaging needs a Mac"
        return 0
    fi
    [[ $PGO == 1 ]] && warn "--pgo is Linux-only for now; building macOS without PGO"
    mkdir -p "$dir"
    local args=(--out "$dir")
    [[ $MODE == debug ]] && args+=(--debug)
    [[ $SIGN == 1 ]] && args+=(--sign)
    [[ -n $FROM ]] && args+=(--from "$FROM")
    if [[ -n $SLICE ]]; then
        step "macos: $SLICE half of the universal binary ($MODE)"
        packaging/macos/build-dmg.sh "${args[@]}" --slice "$SLICE"
        result macos "$dir/$SLICE/specular" "built ($(size_of "$dir/$SLICE/specular"))"
        ok "$(rel "$dir/$SLICE/specular") $C_DIM($(size_of "$dir/$SLICE/specular"))$C_RESET"
        return 0
    fi
    if [[ $PACKAGE == 0 ]]; then
        step "macos: universal binary ($MODE)"
        packaging/macos/build-dmg.sh "${args[@]}" --bin-only
        result macos "$dir/specular$SUFFIX" "built ($(size_of "$dir/specular$SUFFIX"))"
        result macos "$dir/specular-decode-host" "built ($(size_of "$dir/specular-decode-host"))"
        ok "$(rel "$dir/specular$SUFFIX") $C_DIM($(size_of "$dir/specular$SUFFIX"))$C_RESET"
        ok "$(rel "$dir/specular-decode-host") $C_DIM($(size_of "$dir/specular-decode-host"))$C_RESET"
        return 0
    fi
    step "macos: universal binary, .app, .dmg ($MODE)"
    run_pkg macos "$dir/Specular-$VERSION-universal$SUFFIX.dmg" packaging/macos/build-dmg.sh "${args[@]}"
}

# --- run ----------------------------------------------------------------------------------------
case "$PLATFORM" in
    linux) build_linux ;;
    windows) build_windows ;;
    macos) build_macos ;;
    all) build_linux; build_windows; build_macos ;;
esac

step "summary"
# A table of every artifact: platform in its colour, path, and the status with a symbol.
w1=8; w2=8
for r in "${RESULTS[@]}"; do
    p=${r%%|*}; rest=${r#*|}; a=${rest%%|*}
    a=$(rel "$a")
    (( ${#p} > w1 )) && w1=${#p}
    (( ${#a} > w2 )) && w2=${#a}
done
n_ok=0; n_skip=0; n_fail=0
printf '\n  %s%-*s  %-*s  %s%s\n' "$C_DIM" "$w1" platform "$w2" artifact status "$C_RESET"
for r in "${RESULTS[@]}"; do
    p=${r%%|*}; rest=${r#*|}; a=${rest%%|*}; st=${rest#*|}
    case "$st" in
        built*) sym="$C_GREEN$C_BOLD$S_OK$C_RESET"; st="$C_GREEN${st#built }$C_RESET"; n_ok=$((n_ok + 1)) ;;
        skipped*) sym="$C_YELLOW$S_SKIP$C_RESET"; st="$C_YELLOW${st#skipped: }$C_RESET"; n_skip=$((n_skip + 1)) ;;
        *) sym="$C_RED$C_BOLD$S_FAIL$C_RESET"; st="$C_RED$st$C_RESET"; n_fail=$((n_fail + 1)) ;;
    esac
    printf '  %s%-*s%s  %-*s  %s %s\n' "$(platform_color "$p")" "$w1" "$p" "$C_RESET" \
        "$w2" "$(rel "$a")" "$sym" "$st"
done
took=$(elapsed $((SECONDS - START)))
counts="$n_ok built"
(( n_skip > 0 )) && counts+=", $n_skip skipped"
(( n_fail > 0 )) && counts+=", $n_fail failed"
echo
if [[ $FAILED == 1 ]]; then
    die "some packaging steps failed ($counts, $took; see the output above)"
fi
printf '  %s%s Done%s in %s %s(%s)%s\n\n' "$C_GREEN$C_BOLD" "$S_OK" "$C_RESET" "$took" "$C_DIM" "$counts" "$C_RESET"
