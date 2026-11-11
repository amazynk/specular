#!/usr/bin/env bash
# Profile-guided (and optionally BOLT) release build for Linux x86-64.
# DESIGN.md §12.2 "Build".
#
#   tools/release/pgo.sh [--bolt|--bolt-only] [--baseline] [--no-app] [--no-host] [--train-runs N]
#                        [--jpeg-manifest FILE] [--reuse-profile] [--keep-build-dirs] [--out DIR]
#
# Steps:
#   1. llvm-profdata from the `llvm-tools` component of the pinned toolchain (installed if
#      missing; its LLVM must match rustc's, which is checked).
#   2. A training corpus generated into $OUT/corpus (deterministic; nothing from tools/corpus):
#      JPEG (baseline/progressive, 4:2:0/4:4:4, restart markers, greyscale, a 50 MP file), PNG
#      (8/16-bit, alpha), WebP (lossy, lossless graphic and photo), AVIF, HEIC, JPEG XL (lossy and
#      lossless) and TIFF (LZW, Deflate, JPEG) and GIF, from magick, cjpeg, cwebp, heif-enc and
#      ffmpeg (libjxl). A format whose encoder is missing is skipped with a note. The committed
#      JPEG XL test files are added too, except the two the perf gate measures. A DNG (LJ92
#      tiles; no common tool writes one) and a PNG from the png crate's encoder (another deflate
#      stream than magick's zlib) come from `specular-bench gate train` once the instrumented
#      binary exists, at the start of step 4. AVIF comes at 3 MP and 12 MP, so thumbnails train
#      both the 1/4 and the 1/8 decode scale.
#   3. Instrumented build (`-Cprofile-generate`) of `specular`, the specular-bench gate binary and
#      the specular-jpeg `bench-jpeg` example, profile `release-pgo`, explicit `--target` so build
#      scripts and proc macros are not instrumented.
#   4. Training, all on the generated corpus:
#        - `bench-jpeg` on every JPEG (1/1..1/8 scales, auto and single-thread, restart index and
#          region decode), plus the files of --jpeg-manifest (e.g. tools/corpus/out/manifest.json,
#          read in place) if given;
#        - `specular-bench gate run CASE FILE` on every corpus file of the case's format;
#        - `specular --thumbnail FILE OUT 256` on every file (desktop thumbnailer path);
#        - `specular --thumbnail-cache DIR` on the corpus (gallery thumbnailing, cold cache);
#        - `specular --bench-open FILE` (Source, scheduler, decode, GPU upload, tiles) on every
#          file inside a private headless sway, so nothing appears on the user's desktop. Skipped
#          with --no-app or if sway is missing.
#      Every app run gets a private HOME and XDG_{CONFIG,CACHE,DATA,STATE,RUNTIME}_* under $OUT,
#      no DISPLAY and no session bus: the user's settings, caches and running viewer are never
#      touched.
#   5. `llvm-profdata merge`, then the raw profiles and (unless --keep-build-dirs) the
#      instrumented target dir are deleted.
#   6. Optimised build with `-Cprofile-use`.
#   7. --bolt: relink with `--emit-relocs`, instrument with llvm-bolt, re-run the training, and
#      write `*.bolt` binaries. Needs `llvm-bolt` on PATH, in $LLVM_BOLT_DIR, or in
#      target/tools/llvm-bolt of this checkout or (from a git worktree) of the main checkout;
#      it is looked up before anything is built. BOLT needs relocations in the binary, so this
#      build links with ld.lld (or GNU ld) even if the user's cargo config selects another linker
#      (wild 0.10, for one, rejects --emit-relocs). --bolt-only skips steps 3-6 and BOLTs the
#      binaries already in $OUT/dist (from an earlier --bolt run).
#   8. --baseline: the same packages without PGO (the plain `release` settings) into
#      $OUT/dist/plain, for tools/release/pgo-bench.py.
# --reuse-profile skips steps 3-5 and uses an existing $OUT/merged.profdata (same sources only).
#
# Flags are passed with `--config target.<triple>.rustflags=[...]`, which cargo appends to the
# rustflags of .cargo/config.toml (x86-64-v3 stays). RUSTFLAGS would replace them instead.
# The instrumented and PGO builds run without RUSTC_WRAPPER: a compiler cache would key a
# `-Cprofile-use` compile on the profile's path, not its contents.
#
# Output: $OUT/dist/{specular,specular-bench,bench}[.bolt], $OUT/dist/specular-decode-host (not
# PGO-optimised), $OUT/dist/plain/ (--baseline), $OUT/merged.profdata, logs in $OUT/logs.
# Environment: CARGO_TARGET_DIR is ignored (the script uses $OUT/gen, $OUT/use, $OUT/plain and
# $OUT/host); CARGO_BUILD_JOBS is honoured; TMPDIR defaults to $OUT/tmp so nothing lands in /tmp.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/target/pgo"
APP=1
BOLT=0
BASELINE=0
BOLT_ONLY=0
HOST=1
TRAIN_RUNS=1
REUSE=0
KEEP=0
JPEG_MANIFEST=
while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-app) APP=0 ;;
        --no-host) HOST=0 ;;
        --bolt) BOLT=1 ;;
        --bolt-only) BOLT=1; BOLT_ONLY=1 ;;
        --baseline) BASELINE=1 ;;
        --train-runs) TRAIN_RUNS=$2; shift ;;
        --jpeg-manifest) JPEG_MANIFEST=$(realpath "$2"); shift ;;
        --reuse-profile) REUSE=1 ;;
        --keep-build-dirs) KEEP=1 ;;
        --out) OUT=$(realpath -m "$2"); shift ;;
        -h|--help) sed -n '2,55p' "$0"; exit 0 ;;
        *) echo "pgo.sh: unknown option $1" >&2; exit 2 ;;
    esac
    shift
done

cd "$ROOT"
TRIPLE=x86_64-unknown-linux-gnu
PROFILE=release-pgo
CORPUS="$OUT/corpus"
PROF_DIR="$OUT/profraw"
MERGED="$OUT/merged.profdata"
LOGS="$OUT/logs"
export TMPDIR="${TMPDIR_OVERRIDE:-$OUT/tmp}"
mkdir -p "$OUT" "$LOGS" "$TMPDIR" "$OUT/dist"

log() { printf '[pgo %(%H:%M:%S)T] %s\n' -1 "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- 1. llvm-tools ------------------------------------------------------------------------------
TOOLCHAIN=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)
if ! SYSROOT=$(rustc --print sysroot) || [[ ! -x "$SYSROOT/lib/rustlib/$TRIPLE/bin/llvm-profdata" ]]; then
    log "installing llvm-tools for $TOOLCHAIN"
    rustup component add llvm-tools --toolchain "$TOOLCHAIN"
    SYSROOT=$(rustc --print sysroot)
fi
PROFDATA="$SYSROOT/lib/rustlib/$TRIPLE/bin/llvm-profdata"
RUSTC_LLVM=$(rustc -vV | sed -n 's/^LLVM version: \([0-9]*\).*/\1/p')
PROFDATA_LLVM=$("$PROFDATA" --version | sed -n 's/.*LLVM version \([0-9]*\).*/\1/p' | head -1)
[[ "$RUSTC_LLVM" == "$PROFDATA_LLVM" ]] \
    || { log "llvm-profdata is LLVM $PROFDATA_LLVM but rustc uses LLVM $RUSTC_LLVM"; exit 1; }

# The same packages, targets and features in every build, so that the instrumented and optimised
# builds see identical code (PGO matches functions by mangled name and CFG hash), and so that the
# plain baseline is the same program. `specular/asm` (rav1d's and rav1e's assembly for AVIF decode
# and output) needs nasm.
have nasm || { log "nasm not found: it builds rav1d's and rav1e's assembly (apt install nasm)"; exit 1; }
build() { # build <target-dir> <extra rustflags as a TOML array body> [cargo args]
    local dir=$1 flags=$2; shift 2
    CARGO_TARGET_DIR="$dir" cargo build --locked --profile "$PROFILE" --target "$TRIPLE" \
        --config "target.$TRIPLE.rustflags=[$flags]" \
        -p specular -p specular-jpeg -p specular-bench --features specular-bench/gate,specular/asm \
        --bin specular --bin specular-bench --example bench-jpeg "$@"
}
bin_dir() { echo "$1/$TRIPLE/$PROFILE"; }
# dist <build dir> <dest dir> [suffix]
dist() {
    local b; b=$(bin_dir "$1"); mkdir -p "$2"
    cp "$b/specular" "$2/specular"
    cp "$b/specular-bench" "$2/specular-bench"
    cp "$b/examples/bench-jpeg" "$2/bench"
}

# --- 2. training corpus -------------------------------------------------------------------------
# Deterministic for a given set of tool versions (magick's plasma is seeded). The PPM/PNG sources
# go to $OUT/corpus-src (deleted afterwards), outside the corpus folder the gallery run scans.
gen_corpus() {
    local c=$CORPUS s="$OUT/corpus-src"
    if [[ -f "$c/.done" ]]; then log "corpus: reusing $c"; return; fi
    rm -rf "$c" "$s"; mkdir -p "$s" "$c"/{jpeg,png,webp,avif,heic,jxl,tiff,gif}
    have magick || { log "corpus: magick (ImageMagick 7) is needed to generate the corpus"; exit 1; }
    log "corpus: generating sources"
    # Photo-like: fractal plasma, softened, over a warm gradient. Graphic: flat shapes with alpha.
    magick -seed 1 -size 4000x3000 plasma:fractal -blur 0x0.7 \
        \( -size 4000x3000 gradient:'#203040-#e0c090' \) -compose overlay -composite \
        -depth 8 "$s/photo-12mp.ppm"
    magick -seed 2 -size 8160x6120 plasma:fractal -blur 0x0.9 -depth 8 "$s/photo-50mp.ppm"
    magick "$s/photo-12mp.ppm" -resize 2000x1500 "$s/photo-3mp.ppm"
    magick "$s/photo-3mp.ppm" "$s/photo-3mp.png"
    magick -size 1600x1200 xc:none -fill '#3060c0c0' -draw 'rectangle 100,100 900,700' \
        -fill '#e04020' -draw 'circle 1100,600 1100,950' -fill '#20a060a0' \
        -draw 'polygon 200,1100 800,800 1500,1150' "$s/graphic.png"
    log "corpus: encoding"
    if have cjpeg; then
        cjpeg -quality 90 "$s/photo-12mp.ppm" >"$c/jpeg/base-420-q90.jpg"
        cjpeg -quality 90 -progressive "$s/photo-12mp.ppm" >"$c/jpeg/prog-420-q90.jpg"
        cjpeg -quality 95 -sample 1x1 "$s/photo-12mp.ppm" >"$c/jpeg/base-444-q95.jpg"
        cjpeg -quality 85 -sample 1x1 -progressive "$s/photo-3mp.ppm" >"$c/jpeg/prog-444-q85.jpg"
        cjpeg -quality 88 -restart 1 "$s/photo-12mp.ppm" >"$c/jpeg/rst-420-q88.jpg"
        cjpeg -quality 90 -grayscale "$s/photo-3mp.ppm" >"$c/jpeg/grey-q90.jpg"
        cjpeg -quality 92 "$s/photo-50mp.ppm" >"$c/jpeg/big-50mp-q92.jpg"
    else
        log "corpus: cjpeg missing, JPEG via magick (baseline and progressive only)"
        magick "$s/photo-12mp.ppm" -quality 90 "$c/jpeg/base-420-q90.jpg"
        magick "$s/photo-12mp.ppm" -quality 90 -interlace JPEG "$c/jpeg/prog-420-q90.jpg"
        magick "$s/photo-50mp.ppm" -quality 92 "$c/jpeg/big-50mp-q92.jpg"
    fi
    magick "$s/photo-3mp.ppm" "$c/png/rgb8.png"
    magick "$s/photo-3mp.ppm" -depth 16 "$c/png/rgb16.png"
    cp "$s/graphic.png" "$c/png/rgba8-graphic.png"
    if have cwebp; then
        cwebp -quiet -q 80 -m 4 "$s/photo-3mp.png" -o "$c/webp/lossy.webp"
        cwebp -quiet -q 75 "$s/graphic.png" -o "$c/webp/lossy-alpha.webp"
        cwebp -quiet -lossless -z 6 "$s/graphic.png" -o "$c/webp/lossless.webp"
        # A photo uses the predictor and cross-colour transforms a flat graphic leaves out.
        cwebp -quiet -lossless -z 6 "$s/photo-3mp.png" -o "$c/webp/lossless-photo.webp"
    else log "corpus: cwebp missing, no WebP"; fi
    if have heif-enc; then
        heif-enc --avif -q 60 -p chroma=420 -o "$c/avif/420.avif" "$s/photo-3mp.png" >/dev/null
        heif-enc --avif -q 70 -b 10 -p chroma=444 -o "$c/avif/444-10bit.avif" "$s/photo-3mp.png" >/dev/null
        # Big enough that `--thumbnail 256` decodes at 1/8 (the 3 MP files decode at 1/4).
        magick "$s/photo-12mp.ppm" "$s/photo-12mp.png"
        heif-enc --avif -q 55 -p chroma=420 -o "$c/avif/420-12mp.avif" "$s/photo-12mp.png" >/dev/null
        heif-enc -q 60 -o "$c/heic/420.heic" "$s/photo-3mp.png" >/dev/null
        heif-enc -q 50 -o "$c/heic/alpha.heic" "$s/graphic.png" >/dev/null
    else log "corpus: heif-enc missing, no AVIF/HEIC"; fi
    # (Not `ffmpeg | grep -q`: grep exits at the first match, ffmpeg dies of SIGPIPE, and
    # pipefail makes the test false.)
    local encoders=; have ffmpeg && encoders=$(ffmpeg -hide_banner -encoders 2>/dev/null || true)
    if [[ "$encoders" == *' libjxl '* ]]; then
        ffmpeg -y -loglevel error -i "$s/photo-3mp.png" -c:v libjxl -distance 1.0 -frames:v 1 -update 1 "$c/jxl/vardct-d1.jxl"
        ffmpeg -y -loglevel error -i "$s/photo-3mp.png" -c:v libjxl -distance 0 -effort 3 -frames:v 1 -update 1 "$c/jxl/lossless.jxl"
        ffmpeg -y -loglevel error -i "$s/graphic.png" -c:v libjxl -distance 1.5 -frames:v 1 -update 1 "$c/jxl/alpha-d1.5.jxl"
    else log "corpus: ffmpeg with libjxl missing, JPEG XL only from the committed test files"; fi
    local f
    for f in crates/formats/specular-jxl/tests/data/*.jxl; do
        # The perf gate measures these two; keep them out of the training.
        case "${f##*/}" in vardct_multigroup.jxl|modular.jxl) continue ;; esac
        cp "$f" "$c/jxl/test-${f##*/}"
    done
    magick "$s/photo-3mp.ppm" -compress LZW "$c/tiff/lzw.tif"
    magick "$s/photo-3mp.ppm" -compress Zip "$c/tiff/deflate.tif"
    magick "$s/photo-3mp.ppm" -compress JPEG -quality 90 "$c/tiff/jpeg.tif"
    magick "$s/photo-3mp.ppm" -resize 50% -colors 256 "$c/gif/still.gif"
    rm -rf "$s"
    touch "$c/.done"
    log "corpus: $(corpus_files | wc -l) files, $(du -sh "$c" | cut -f1)"
}
corpus_files() { find "$CORPUS" -type f ! -name '.*' | sort; }

# The perf-gate case that decodes a file of this format, if any.
gate_case() {
    case "$1" in
        */jpeg/*) echo jpeg-420-3mp ;;
        */png/*) echo png-rgb8-photo ;;
        */webp/*) echo webp-lossy ;;
        */avif/*) echo avif-420 ;;
        */heic/*) echo heic-grid ;;
        */jxl/*) echo jxl-vardct ;;
        */tiff/*) echo tiff-lzw ;;
        */gif/*) echo gif-still ;;
        */raw/*) echo raw-dng-lj92 ;;
    esac
}

# --- private environment for app runs ----------------------------------------------------------
PRIV="$OUT/home"
private_env() {
    mkdir -p "$PRIV"/{config,cache,data,state} "$OUT/xdg"; chmod 700 "$OUT/xdg"
    APP_ENV=(env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS -u SPECULAR_LOG_FILE
        HOME="$PRIV" XDG_CONFIG_HOME="$PRIV/config" XDG_CACHE_HOME="$PRIV/cache"
        XDG_DATA_HOME="$PRIV/data" XDG_STATE_HOME="$PRIV/state" XDG_RUNTIME_DIR="$OUT/xdg")
}
private_env

# Private headless Wayland compositor for the app run. Its socket lives in $OUT/xdg, not /tmp.
SWAY_PID=
WAYLAND_ENV=()
start_compositor() {
    have sway || { log "sway not found: skipping the app training run"; return 1; }
    printf 'xwayland disable\noutput HEADLESS-1 resolution 2560x1440\n' >"$OUT/sway.conf"
    rm -f "$OUT/xdg"/wayland-* "$OUT/xdg"/sway-ipc.*
    "${APP_ENV[@]}" WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 \
        sway -c "$OUT/sway.conf" >"$LOGS/sway.log" 2>&1 &
    SWAY_PID=$!
    local _ sock
    for _ in $(seq 50); do
        for sock in "$OUT/xdg"/wayland-[0-9]*; do
            [[ -S "$sock" ]] || continue
            WAYLAND_ENV=(WAYLAND_DISPLAY="${sock##*/}")
            return 0
        done
        sleep 0.1
    done
    log "headless sway did not start (see $LOGS/sway.log)"; stop_compositor; return 1
}
stop_compositor() {
    if [[ -n "$SWAY_PID" ]]; then kill "$SWAY_PID" 2>/dev/null || true; wait "$SWAY_PID" 2>/dev/null || true; SWAY_PID=; fi
}
trap stop_compositor EXIT

# train <dir with specular, specular-bench and examples/bench-jpeg>
train() {
    local bins=$1 n=0 fail=0 f case
    if [[ ! -s "$CORPUS/raw/lj92.dng" ]]; then
        # Written by the binary being trained, so its own profile goes elsewhere and is dropped.
        LLVM_PROFILE_FILE="$OUT/tmp/dng-gen.profraw" "$bins/specular-bench" gate train "$CORPUS" \
            >>"$LOGS/train-gate.log" 2>&1 || log "  could not write the training DNG"
        rm -f "$OUT/tmp/dng-gen.profraw"
    fi
    log "training: bench-jpeg on the corpus JPEGs"
    {
        corpus_files | grep '/jpeg/'
        if [[ -n "$JPEG_MANIFEST" ]]; then
            python3 -I - "$JPEG_MANIFEST" <<'EOF'
import json, os, sys
m = json.load(open(sys.argv[1]))
base = os.path.dirname(sys.argv[1])
for f in m["files"]:
    if not f.get("expected_error"):
        p = os.path.normpath(os.path.join(base, f["path"]))
        if os.path.isfile(p):
            print(p)
EOF
        fi
    } | while read -r f; do
        BENCH_RUNS=$TRAIN_RUNS BENCH_NO_EXTERNAL=1 "$bins/examples/bench-jpeg" "$f" >>"$LOGS/train-bench.log" 2>&1 \
            || log "  bench-jpeg failed on $f"
    done
    log "training: specular-bench gate decodes"
    while read -r f; do
        case=$(gate_case "$f"); [[ -n "$case" ]] || continue
        "$bins/specular-bench" gate run "$case" "$f" >>"$LOGS/train-gate.log" 2>&1 || log "  gate $case failed on $f"
    done < <(corpus_files)
    log "training: specular --thumbnail"
    mkdir -p "$TMPDIR/thumbs"
    while read -r f; do
        n=$((n + 1))
        if ! timeout 120 "${APP_ENV[@]}" "$bins/specular" --thumbnail "$f" "$TMPDIR/thumbs/$n.png" 256 \
            >>"$LOGS/train-thumb.log" 2>&1; then
            fail=$((fail + 1)); log "  thumbnail failed on $f"
        fi
    done < <(corpus_files)
    log "  $n files, $fail failures"
    log "training: specular --thumbnail-cache (gallery)"
    rm -rf "$PRIV/cache/specular"
    timeout 600 "${APP_ENV[@]}" "$bins/specular" --thumbnail-cache "$CORPUS" --depth 1 \
        >>"$LOGS/train-gallery.log" 2>&1 || log "  gallery thumbnailing failed (see $LOGS/train-gallery.log)"
    if [[ $APP == 1 ]] && start_compositor; then
        log "training: specular --bench-open (headless sway)"
        while read -r f; do
            timeout 120 "${APP_ENV[@]}" "${WAYLAND_ENV[@]}" "$bins/specular" --size 1920x1080 --bench-open "$f" \
                >>"$LOGS/train-app.log" 2>&1 || log "  app run failed on $f (see $LOGS/train-app.log)"
        done < <(corpus_files)
        stop_compositor
    fi
}

# Find llvm-bolt first, so a missing one is reported before an hour of building.
BOLT_BIN=
if [[ $BOLT == 1 ]]; then
    MAIN_ROOT=$(cd "$(git rev-parse --git-common-dir 2>/dev/null || echo .git)/.." 2>/dev/null && pwd || echo "$ROOT")
    for c in "$(command -v llvm-bolt || true)" "${LLVM_BOLT_DIR:+$LLVM_BOLT_DIR/bin/llvm-bolt}" \
        "$ROOT/target/tools/llvm-bolt/bin/llvm-bolt" "$MAIN_ROOT/target/tools/llvm-bolt/bin/llvm-bolt"; do
        [[ -n "$c" && -x "$c" ]] && { BOLT_BIN=$c; break; }
    done
    if [[ -n "$BOLT_BIN" ]]; then
        log "BOLT: $BOLT_BIN ($("$BOLT_BIN" --version | sed -n 's/.*LLVM version //p' | head -1))"
    else
        log "llvm-bolt not found (PATH, \$LLVM_BOLT_DIR or target/tools/llvm-bolt): no BOLT"
        [[ $BOLT_ONLY == 1 ]] && exit 1
        BOLT=0
    fi
fi

gen_corpus

if [[ $BOLT_ONLY == 1 ]]; then
    [[ -x "$OUT/dist/specular" ]] || { log "--bolt-only: no binaries in $OUT/dist"; exit 1; }
    readelf -S "$OUT/dist/specular" | grep -q '\.rela\.text' \
        || { log "--bolt-only: $OUT/dist/specular has no relocations (build it with --bolt)"; exit 1; }
else
if [[ $REUSE == 1 && -s "$MERGED" ]]; then
    log "reusing $MERGED"
else
# --- 3. instrumented build ----------------------------------------------------------------------
rm -rf "$PROF_DIR"; mkdir -p "$PROF_DIR"
log "instrumented build"
SECONDS=0
RUSTC_WRAPPER='' build "$OUT/gen" "\"-Cprofile-generate=$PROF_DIR\"" >"$LOGS/build-gen.log" 2>&1 \
    || { tail -30 "$LOGS/build-gen.log"; exit 1; }
log "  ${SECONDS}s"

# --- 4. training --------------------------------------------------------------------------------
SECONDS=0
train "$(bin_dir "$OUT/gen")"
log "  training ${SECONDS}s"

# --- 5. merge -----------------------------------------------------------------------------------
shopt -s nullglob
raws=("$PROF_DIR"/*.profraw)
[[ ${#raws[@]} -gt 0 ]] || { log "no .profraw files were written"; exit 1; }
"$PROFDATA" merge -o "$MERGED" "${raws[@]}"
log "merged ${#raws[@]} profiles into $MERGED ($(du -h "$MERGED" | cut -f1))"
rm -rf "$PROF_DIR"
[[ $KEEP == 1 ]] || rm -rf "$OUT/gen"
fi

# --- 6. optimised build -------------------------------------------------------------------------
# -pgso=false: without it LLVM optimises every block the training never reached for size, and
# its loop vectoriser then gives up on any loop that needs a remainder or a runtime check, so
# untrained paths ran up to 2x slower than in the plain build (WebP's cross-colour transform).
USE_FLAGS="\"-Cprofile-use=$MERGED\", \"-Cllvm-args=-pgo-warn-missing-function=false\", \"-Cllvm-args=-pgso=false\""
if [[ $BOLT == 1 ]]; then
    BOLT_LD=$(command -v ld.lld || command -v ld.bfd || true)
    [[ -n "$BOLT_LD" ]] || { log "--bolt needs ld.lld or GNU ld (apt install lld)"; exit 1; }
    # clang uses the last --ld-path; --config rustflags come after the user's cargo config.
    USE_FLAGS="$USE_FLAGS, \"-Clink-arg=-Wl,--emit-relocs\", \"-Clink-arg=--ld-path=$BOLT_LD\""
fi
log "PGO build"
SECONDS=0
RUSTC_WRAPPER='' build "$OUT/use" "$USE_FLAGS" >"$LOGS/build-use.log" 2>&1 || { tail -30 "$LOGS/build-use.log"; exit 1; }
log "  ${SECONDS}s"
dist "$OUT/use" "$OUT/dist"
log "PGO binaries in $OUT/dist"

# The sandboxed decode host ships next to the viewer. Plain release build (same profile and
# target, own target dir): its time goes to libheif and UnRAR, and an instrumented host could
# not write its profile from inside its sandbox anyway.
if [[ $HOST == 1 ]]; then
    log "decode host build"
    CARGO_TARGET_DIR="$OUT/host" cargo build --locked --profile "$PROFILE" --target "$TRIPLE" \
        -p specular-decode-host --bin specular-decode-host >"$LOGS/build-host.log" 2>&1 \
        || { tail -30 "$LOGS/build-host.log"; exit 1; }
    cp "$OUT/host/$TRIPLE/$PROFILE/specular-decode-host" "$OUT/dist/specular-decode-host"
    [[ $KEEP == 1 ]] || rm -rf "$OUT/host"
fi
fi # BOLT_ONLY

# --- 7. BOLT ------------------------------------------------------------------------------------
if [[ $BOLT == 1 ]]; then
    {
        MERGE_FDATA="$(dirname "$BOLT_BIN")/merge-fdata"
        BOLT_DIR="$OUT/bolt"; rm -rf "$BOLT_DIR"; mkdir -p "$BOLT_DIR/fdata" "$BOLT_DIR/inst/examples"
        SECONDS=0
        for b in specular specular-bench bench; do
            dst="$BOLT_DIR/inst/$b"; [[ $b == bench ]] && dst="$BOLT_DIR/inst/examples/bench-jpeg"
            log "BOLT instrument $b"
            "$BOLT_BIN" "$OUT/dist/$b" -instrument -instrumentation-file="$BOLT_DIR/fdata/$b" \
                -instrumentation-file-append-pid -o "$dst" >"$LOGS/bolt-inst-$b.log" 2>&1 \
                || { tail -20 "$LOGS/bolt-inst-$b.log"; exit 1; }
        done
        train "$BOLT_DIR/inst"
        for b in specular specular-bench bench; do
            shopt -s nullglob
            fd=("$BOLT_DIR/fdata/$b".*.fdata)
            [[ ${#fd[@]} -gt 0 ]] || { log "no BOLT profile for $b"; continue; }
            "$MERGE_FDATA" "${fd[@]}" -o "$BOLT_DIR/$b.fdata" 2>>"$LOGS/bolt-merge.log"
            log "BOLT optimise $b (${#fd[@]} profiles)"
            "$BOLT_BIN" "$OUT/dist/$b" -o "$OUT/dist/$b.bolt" -data="$BOLT_DIR/$b.fdata" \
                -reorder-blocks=ext-tsp -reorder-functions=cdsort -split-functions -split-all-cold \
                -split-eh -icf=1 -use-gnu-stack -dyno-stats >"$LOGS/bolt-opt-$b.log" 2>&1 \
                || { tail -20 "$LOGS/bolt-opt-$b.log"; exit 1; }
        done
        rm -rf "$BOLT_DIR/inst" "$BOLT_DIR/fdata"
        log "BOLT binaries in $OUT/dist (*.bolt), ${SECONDS}s"
    }
fi
[[ $KEEP == 1 ]] || rm -rf "$OUT/use"

# --- 8. plain baseline --------------------------------------------------------------------------
if [[ $BASELINE == 1 ]]; then
    log "plain build (no PGO) for comparison"
    SECONDS=0
    build "$OUT/plain" "" >"$LOGS/build-plain.log" 2>&1 || { tail -30 "$LOGS/build-plain.log"; exit 1; }
    log "  ${SECONDS}s"
    dist "$OUT/plain" "$OUT/dist/plain"
    [[ $KEEP == 1 ]] || rm -rf "$OUT/plain"
fi
