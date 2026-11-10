#!/usr/bin/env bash
# Type-checks (and lints) the workspace for the Windows and macOS targets from a Linux (or any
# non-native) host, without a C toolchain for those targets.
#
#   tools/cross-check.sh [check|clippy|both] [TARGET...] [-- CARGO_ARGS...]
#
#   check     cargo check --all-targets (default)
#   clippy    cargo clippy --all-targets -- -D warnings
#   both      check, then clippy
#   TARGET    default: x86_64-pc-windows-msvc aarch64-apple-darwin
#   CARGO_ARGS passed to cargo before the clippy `--`, e.g. `-- -p specular -p specular-sandbox`
#             (default: --workspace)
#
# Some crates build C/C++ in their build scripts (libmimalloc-sys for the viewer's allocator,
# unrar_sys in specular-decode-host). `cargo check` still runs those build scripts, which fail
# without a compiler and archiver for the target. This script points CC_<target>, CXX_<target>
# and AR_<target> at stand-in scripts that only create the empty object files and archives the
# build scripts expect. Nothing is linked by check/clippy, so the result is the same as with a
# real toolchain, except that C compile errors are not seen.
#
# For local use only: CI checks and tests natively on each OS (.github/workflows/ci.yml).
#
# Environment: CARGO_TARGET_DIR (default target/cross-check, so the stub-built C objects never
# mix with real ones), CARGO_BUILD_JOBS, as usual for cargo.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/cross-check}"
# Absolute: CC_<target> points into it, and build scripts run in their own crate's directory.
[[ $CARGO_TARGET_DIR == /* ]] || CARGO_TARGET_DIR="$ROOT/$CARGO_TARGET_DIR"

MODE=check
TARGETS=()
EXTRA=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        check|clippy|both) MODE=$1 ;;
        --) shift; EXTRA=("$@"); break ;;
        -h|--help) sed -n '2,24p' "$0"; exit 0 ;;
        -*) echo "cross-check.sh: unknown option $1 (cargo arguments go after --)" >&2; exit 2 ;;
        *) TARGETS+=("$1") ;;
    esac
    shift
done
[[ ${#TARGETS[@]} -gt 0 ]] || TARGETS=(x86_64-pc-windows-msvc aarch64-apple-darwin)
[[ ${#EXTRA[@]} -gt 0 ]] || EXTRA=(--workspace)

# --- stand-in compiler and archiver --------------------------------------------------------------
STUBS="$CARGO_TARGET_DIR/cross-stubs"
mkdir -p "$STUBS"
# Compiler (GCC, Clang or MSVC style flags): creates the output file of `-o FILE`, `-oFILE`,
# `-Fo FILE`, `-FoFILE` or `/FoFILE`; preprocessing (-E, used by cc to detect the compiler
# family) prints nothing; `--version` and the like succeed.
cat >"$STUBS/cc" <<'EOF'
#!/usr/bin/env bash
out=
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o|-Fo|/Fo) out=${2:-}; shift ;;
        -Fo*|/Fo*) out=${1:3} ;;
        -o*) out=${1:2} ;;
        -E|-M|-MM) exit 0 ;;
    esac
    shift
done
[[ -n "$out" ]] && : >"$out"
exit 0
EOF
# Archiver (ar `crs LIB.a objs...` or lib.exe `-out:LIB` / `/OUT:LIB`): writes an empty archive.
cat >"$STUBS/ar" <<'EOF'
#!/usr/bin/env bash
out=
for a in "$@"; do
    case "$a" in
        -out:*|-OUT:*|/out:*|/OUT:*) out=${a#*:} ;;
        *.a|*.lib) [[ -z "$out" ]] && out=$a ;;
    esac
done
[[ -n "$out" ]] && printf '!<arch>\n' >"$out"
exit 0
EOF
chmod +x "$STUBS/cc" "$STUBS/ar"

run() {
    local target=$1; shift
    local v=${target//-/_}
    rustup target list --installed 2>/dev/null | grep -qx "$target" \
        || { echo "cross-check.sh: Rust target $target is not installed: rustup target add $target" >&2; return 1; }
    printf '\n== %s: %s\n' "$target" "$*"
    env "CC_$v=$STUBS/cc" "CXX_$v=$STUBS/cc" "AR_$v=$STUBS/ar" \
        "CFLAGS_$v=" "CXXFLAGS_$v=" \
        "$@"
}

status=0
for t in "${TARGETS[@]}"; do
    if [[ $MODE == check || $MODE == both ]]; then
        run "$t" cargo check --locked --target "$t" --all-targets "${EXTRA[@]}" || status=1
    fi
    if [[ $MODE == clippy || $MODE == both ]]; then
        run "$t" cargo clippy --locked --target "$t" --all-targets "${EXTRA[@]}" -- -D warnings || status=1
    fi
done
exit $status
