#!/usr/bin/env bash
# Fetch the raw video corpus into tools/corpus/raw-video/ (gitignored: nothing here is committed)
# and write FFmpeg's decoding of the ProRes RAW frames (the oracle) beside it.
#
# Usage: tools/corpus/fetch-raw-video.sh
#   OUT defaults to $SPECULAR_RAW_VIDEO_DIR, else tools/corpus/raw-video; ORACLE (FFmpeg's output)
#   to $SPECULAR_RAW_VIDEO_ORACLE, else $OUT/oracle. FFMPEG (default ffmpeg) must have the
#   prores_raw decoder (FFmpeg 8 or later).
#   Then: SPECULAR_RAW_VIDEO_DIR=tools/corpus/raw-video SPECULAR_RAW_VIDEO_ORACLE=<oracle>
#   cargo test -p specular-prores --test raw_oracle -- --nocapture (ProRes RAW against FFmpeg),
#   cargo test -p specular-raw --test raw_clips -- --nocapture (Cinema RAW Light and ProRes RAW
#   clips: frames, CRX against the CR3 path, the embedded thumbnail) and
#   cargo test -p specular-decode --test raw_clips (sessions, frames, facts).
#
# What it covers (about 46 MB; both clips are on FFmpeg's public sample server):
#   c200_prefix.crm  Canon EOS C200 Cinema RAW Light, 4096×2160 (4112×2176 stored), 10-bit
#                    CRX wavelet (3 levels), 59.94 fps: the first 22.8 MB of the 100 MB
#                    "Canon-C200-Raw_cut.CRM" (FFmpeg trac ticket 6765), fetched with an HTTP
#                    Range request. The movie header comes first, so the prefix holds the
#                    header, the first chunk of 10 frames, sound and metadata; the other frames
#                    the sample table lists are past its end (a clip cut short).
#   fs_aprh_4f.mov   Apple ProRes RAW HQ, 4112×2176 RGGB, Sony camera through an Atomos Shogun
#                    Inferno: the first 4 frames of "Filmplusgear-ProRes-RAW-testfiles-6.mov"
#                    (FFmpeg trac ticket 7887, 1 GB). Its movie header is at the end: the
#                    header and the first 23.5 MB are fetched with Range requests and
#                    tools/corpus/raw-video-cut.py rebuilds a movie of the first 4 frames.
#
# Downloads are untrusted data: this script only copies them, rebuilds box tables and checks
# their SHA-256. Stops if free space drops below MIN_FREE_GB (default 8).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_RAW_VIDEO_DIR:-$ROOT/tools/corpus/raw-video}
ORACLE=${SPECULAR_RAW_VIDEO_ORACLE:-$OUT/oracle}
MIN_FREE_GB=${MIN_FREE_GB:-8}
PYTHON=${PYTHON:-python3}
FFMPEG=${FFMPEG:-ffmpeg}
BASE=https://samples.ffmpeg.org/ffmpeg-bugs/trac
CRM_SHA=33d132b9df04a5beec0a83461e112d0d3f345453866b40b6b50a2a764af4f1f2
PRR_SHA=c4bc7791ef89111879d2dea0bfa4188ee52a579f6b8769a12a2383d4cf228bce
mkdir -p "$OUT" "$OUT/.work" "$ORACLE"

die() { echo "fetch-raw-video: $*" >&2; exit 1; }

check_space() {
    local avail
    avail=$(df --output=avail -BG "$OUT" | tail -1 | tr -dc 0-9)
    [ "$avail" -ge "$MIN_FREE_GB" ] || die "only ${avail} GB free (need ${MIN_FREE_GB})"
}

# fetch URL FIRST-LAST DEST: a byte range, with curl's progress bar (what, speed, size).
fetch() {
    check_space
    echo "fetch-raw-video: $(basename "$3") (bytes $2 of $1)" >&2
    curl -fL --progress-bar -r "$2" -o "$3.part" "$1" && mv "$3.part" "$3"
}

check() {
    local got
    got=$(sha256sum "$1" | cut -d' ' -f1)
    [ "$got" = "$2" ] || die "$(basename "$1"): SHA-256 $got, expected $2"
}

# Canon C200 Cinema RAW Light: the prefix up to the second video chunk (0x15c0310).
if [ ! -f "$OUT/c200_prefix.crm" ]; then
    fetch "$BASE/ticket6765/Canon-C200-Raw_cut.CRM" 0-22807311 "$OUT/c200_prefix.crm"
fi
check "$OUT/c200_prefix.crm" "$CRM_SHA"

# ProRes RAW HQ: the movie header (from 1041307136, after the mdat) and the first 4 frames.
if [ ! -f "$OUT/fs_aprh_4f.mov" ]; then
    url="$BASE/ticket7887/Filmplusgear-ProRes-RAW-testfiles-6.mov"
    fetch "$url" 1041307136- "$OUT/.work/prr_moov.bin"
    fetch "$url" 0-23500287 "$OUT/.work/prr_head.bin"
    "$PYTHON" -I "$ROOT/tools/corpus/raw-video-cut.py" "$OUT/.work/prr_head.bin" \
        "$OUT/.work/prr_moov.bin" 4 "$OUT/fs_aprh_4f.mov"
    rm -f "$OUT/.work/prr_head.bin"
fi
check "$OUT/fs_aprh_4f.mov" "$PRR_SHA"

# The oracle: every frame as FFmpeg decodes it (16-bit little-endian Bayer).
if command -v "$FFMPEG" >/dev/null; then
    for m in "$OUT"/*.mov; do
        o="$ORACLE/$(basename "${m%.mov}").bayer16"
        [ -f "$o" ] || "$FFMPEG" -v error -i "$m" -map 0:v -f rawvideo \
            -pix_fmt bayer_rggb16le "$o"
    done
else
    echo "fetch-raw-video: no $FFMPEG: the ProRes RAW oracle is not written" >&2
fi
rm -rf "$OUT/.work"
echo "fetch-raw-video: done ($(du -sh "$OUT" | cut -f1))" >&2
