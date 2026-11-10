#!/usr/bin/env bash
# Screen-reader rows of docs/accessibility.md on Linux, automated: runs Specular in a private
# headless sway on a private D-Bus session bus with a private AT-SPI bus, and checks what a
# screen reader (Orca) would see through libatspi. Needs sway, wtype, dbus-run-session,
# dbus-daemon, at-spi2-registryd, python3-gi with Atspi 2.0, and lavapipe
# (mesa-vulkan-drivers).
#
#   tools/a11y/atspi-check.sh [path/to/specular]     (default: $CARGO_TARGET_DIR/debug/specular)
#   tools/a11y/atspi-check.sh --ime [path/to/specular]
#   tools/a11y/atspi-check.sh --similar [path/to/specular]
#   tools/a11y/atspi-check.sh --series [path/to/specular]
#
# --ime runs the IME rows instead (ime_check.py): the app on a private Xvfb with a private
# IBus (XIM, Cangjie 5 table engine), typed into with xdotool. Needs Xvfb, xdotool, ibus,
# ibus-x11 and ibus-table with its cangjie5 table.
#
# --similar runs the similar-shots screens instead (similar_check.py): the same sway and buses
# as the default run, on the test images plus byte-identical copies; then again in Hebrew
# (--rtl: bounds mirrored against the first run's, in similar-check/he/), then during a scan
# of generated images (--progress: the progress bar and its polite announcements, in
# similar-check/scan/; SPECULAR_SCAN_IMAGES images, 600 by default, made with Pillow).
#
# --series runs the DICOM series screens instead (series_check.py): a generated CT series in
# the Medical profile, a slice copied in while in series mode, MPR in a full and a small
# window, and the Studies browser, in series-check/.
#
# Never touches the user's session bus, desktop, clipboard or ~/.config: HOME and every XDG
# folder point into $CARGO_TARGET_DIR/a11y-check/ (ime-check/ with --ime, similar-check/ with --similar), which also gets the bus sockets, the trees
# (trees/*.txt), the logs and results.json. Exit status 0 means every check passed.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
check=atspi_check.py
name=a11y-check
tools="sway wtype dbus-run-session dbus-daemon python3"
if [ "${1:-}" = --ime ]; then
    shift
    check=ime_check.py
    name=ime-check
    tools="Xvfb xdotool setxkbmap xmodmap ibus-daemon ibus dbus-run-session dbus-daemon python3"
elif [ "${1:-}" = --similar ]; then
    shift
    check=similar_check.py
    name=similar-check
elif [ "${1:-}" = --series ]; then
    shift
    check=series_check.py
    name=series-check
fi
target=${CARGO_TARGET_DIR:-$root/target}
bin=${1:-$target/debug/specular}
out=$target/$name
[ -x "$bin" ] || { echo "no binary at $bin (cargo build -p specular)"; exit 2; }

for tool in $tools; do
    command -v "$tool" >/dev/null || { echo "missing: $tool"; exit 2; }
done

# One run of $check with HOME, XDG folders and buses under $1; the rest are its arguments.
run() {
local out=$1
shift
rm -rf "$out/home" "$out/run" "$out/images"
mkdir -p "$out/home" "$out/run" "$out/images"
chmod 700 "$out/run"

# Committed, non-private test images; the last one is broken on purpose (the error alert).
cp "$root/crates/formats/specular-jpeg/fuzz/seeds/s420.jpg" "$out/images/beach.jpg"
cp "$root/crates/formats/specular-jpeg/fuzz/seeds/s444.jpg" "$out/images/harbour.jpg"
cp "$root/crates/formats/specular-jpeg/tests/fixtures/gray8.jpg" "$out/images/snow.jpg"
printf 'not a jpeg at all' > "$out/images/zz-broken.jpg"

# The session bus: its socket in the private runtime folder, no service activation.
sed "s|@RUN@|$out/run|" "$root/tools/a11y/session-bus.conf.in" > "$out/session-bus.conf"

env -i PATH="$PATH" LANG=C.UTF-8 HOME="$out/home" XDG_CONFIG_HOME="$out/home/config" XDG_CACHE_HOME="$out/home/cache" XDG_DATA_HOME="$out/home/data" XDG_STATE_HOME="$out/home/state" XDG_RUNTIME_DIR="$out/run" GSETTINGS_BACKEND=memory SPECULAR_A11Y_PRIVATE=1 SPECULAR_SCAN_IMAGES="${SPECULAR_SCAN_IMAGES:-600}" dbus-run-session --config-file="$out/session-bus.conf" -- python3 -I "$root/tools/a11y/$check" --bin "$bin" --out "$out" --images "$out/images" "$@"
}

if [ "$name" = similar-check ]; then
    status=0
    run "$out" || status=1
    run "$out/he" --rtl || status=1
    run "$out/scan" --progress || status=1
    exit $status
fi
run "$out"
