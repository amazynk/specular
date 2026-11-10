#!/usr/bin/env bash
# Real-use runs: the real specular binary driven through user workflows in a private headless
# sway, with a private D-Bus session bus, a private AT-SPI bus and a FileChooser portal that
# answers the app's dialogs (tools/realuse/portal.py). Writes reports, logs and screenshots
# under $CARGO_TARGET_DIR/realuse/<label>/; never commits them.
#
#   tools/realuse/realuse.sh [--bin path/to/specular] [--label L] [--only 1,2] [--lang he]
#                            [--compare OTHER_LABEL]
#
# Needs a debug build (the test commands `idle` and `ui-state`, the mock camera): cargo build
# -p specular. Needs sway, wtype, dbus-run-session, dbus-daemon, at-spi2-registryd,
# python3-gi with Atspi 2.0 and lavapipe (mesa-vulkan-drivers). No network: the app runs with
# SPECULAR_NO_NETWORK=1. HOME and every XDG folder point under the label's folder.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
out=$target/realuse
mkdir -p "$out"
for tool in sway wtype dbus-run-session dbus-daemon python3; do
    command -v "$tool" >/dev/null || { echo "missing: $tool"; exit 2; }
done
# The run's runtime folder (bus sockets, the app's remote-control socket): short, private.
run=$out/run
rm -rf "$run"
mkdir -p "$run" "$out/home"
chmod 700 "$run"
sed "s|@RUN@|$run|" "$root/tools/a11y/session-bus.conf.in" > "$out/session-bus.conf"

exec env -i PATH="$PATH" LANG=C.UTF-8 HOME="$out/home" XDG_CONFIG_HOME="$out/home/config" \
    XDG_CACHE_HOME="$out/home/cache" XDG_DATA_HOME="$out/home/data" \
    XDG_STATE_HOME="$out/home/state" XDG_RUNTIME_DIR="$run" GSETTINGS_BACKEND=memory \
    SPECULAR_A11Y_PRIVATE=1 CARGO_TARGET_DIR="$target" \
    dbus-run-session --config-file="$out/session-bus.conf" -- \
    python3 -I "$root/tools/realuse/run.py" --root "$root" --out "$out" "$@"
