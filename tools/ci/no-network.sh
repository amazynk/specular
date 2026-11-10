#!/usr/bin/env bash
# The test suite with the network blocked: every test binary runs inside a new network
# namespace that has only loopback, so a test that needs the internet (or any server off this
# machine) fails here. Tests against loopback servers (specular_remote::testing::TestServer,
# marked "Loopback only" in their docs) still run: loopback is up inside the namespace.
#
#   tools/ci/no-network.sh [-p CRATE]... [-- TEST-ARGS...]
#
#   -p CRATE    test these packages (default: the whole workspace)
#   TEST-ARGS   passed to every test binary (e.g. --skip slow_thing)
#
# It builds the tests first, as they are (--offline: nothing is fetched), then runs each test
# binary in the namespace from its package's folder, as cargo would. Doc-tests are not run.
# The cargo environment (CARGO_TARGET_DIR, RUSTC_WRAPPER, ...) is used as it is set.
#
# Needs: unshare (util-linux) with unprivileged user namespaces, and ip (iproute2) to bring
# loopback up. Where they are off (kernel.unprivileged_userns_clone=0, or Ubuntu's
# kernel.apparmor_restrict_unprivileged_userns=1), run as root or enable them for CI.
# Exits 0 when every test passed, 1 when any failed, 2 when the namespace is not available.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)

if [[ "${1:-}" == "--inside" ]]; then
    # In the namespace: $2 is the list of "binary<TAB>package dir<TAB>package" lines.
    list=$2
    shift 2
    [[ "${1:-}" == "--" ]] && shift
    ip link set lo up
    # Nothing beyond loopback: an outside address must not connect.
    if (exec 3<>/dev/tcp/1.1.1.1/443) 2>/dev/null; then
        echo "no-network.sh: the namespace still reaches the internet" >&2
        exit 2
    fi
    failed=()
    total=0
    while IFS=$'\t' read -r exe dir pkg; do
        [[ -n "$exe" ]] || continue
        total=$((total + 1))
        echo "== $pkg: $(basename "$exe")"
        if ! (cd "$dir" && CARGO_MANIFEST_DIR="$dir" CARGO_PKG_NAME="$pkg" "$exe" "$@"); then
            failed+=("$pkg: $(basename "$exe")")
        fi
    done <"$list"
    echo
    if ((${#failed[@]})); then
        echo "no-network.sh: ${#failed[@]} of $total test binaries failed without the network:"
        printf '  %s\n' "${failed[@]}"
        exit 1
    fi
    echo "no-network.sh: all $total test binaries passed without the network"
    exit 0
fi

pkgs=()
args=()
while (($#)); do
    case "$1" in
        -p) pkgs+=(-p "$2"); shift 2 ;;
        --) shift; args=("$@"); break ;;
        -h | --help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "no-network.sh: unknown argument: $1" >&2; exit 2 ;;
    esac
done
((${#pkgs[@]})) || pkgs=(--workspace)

if ! command -v unshare >/dev/null || ! unshare -rn true 2>/dev/null; then
    echo "no-network.sh: unshare -rn is not available (unprivileged user namespaces are off)." >&2
    echo "Run as root, or enable them for CI (see the comment at the top of this script)." >&2
    exit 2
fi
command -v ip >/dev/null || { echo "no-network.sh: ip (iproute2) is needed" >&2; exit 2; }

cd "$ROOT"
target=${CARGO_TARGET_DIR:-$ROOT/target}
mkdir -p "$target"
list=$(mktemp "$target/no-network-tests.XXXXXX")
trap 'rm -f "$list"' EXIT
cargo test --offline --no-run --message-format=json "${pkgs[@]}" |
    python3 -I -c '
import json, os, sys, tomllib
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") != "compiler-artifact" or not m.get("executable"):
        continue
    if not m.get("profile", {}).get("test"):
        continue
    with open(m["manifest_path"], "rb") as f:
        name = tomllib.load(f)["package"]["name"]
    print(m["executable"], os.path.dirname(m["manifest_path"]), name, sep="\t")
' >"$list"
[[ -s "$list" ]] || { echo "no-network.sh: no test binaries built" >&2; exit 2; }
status=0
unshare -rn bash "$0" --inside "$list" -- "${args[@]}" || status=$?
exit "$status"
