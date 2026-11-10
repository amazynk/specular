#!/usr/bin/env bash
# Fetch real CineForm samples for specular-cineform's corpus test (gitignored: nothing here is
# committed) and make FFmpeg's decodes of them as the oracle (FFmpeg is a tool only).
#
# Usage: tools/corpus/fetch-vgaps3.sh
#   OUT defaults to $SPECULAR_VGAPS3_CORPUS, else $CARGO_TARGET_DIR/corpus/vgaps3, else
#   target/corpus/vgaps3.
#   Then: SPECULAR_VGAPS3_CORPUS=<OUT> cargo test -p specular-cineform --test corpus -- --ignored
#
# What it fetches (about 33 MB) from samples.ffmpeg.org/V-codecs/CFHD/:
#   - timelapse-pt-momsapt.avi: 1280x720 YUV 4:2:2, 8-bit source, spatial transform,
#     codebook 9 (2008).
#   - MT_BeartoothHighway_1min_Cineform.avi, its first 6 MB: 1280x720 YUV 4:2:2, field+
#     transform, two-frame groups (2005).
#   - lower_res.mov: 3312x2488 Bayer RAW, 12-bit (2015).
# Downloads are untrusted data: this script only copies them and runs FFmpeg on them.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${SPECULAR_VGAPS3_CORPUS:-${CARGO_TARGET_DIR:-$ROOT/target}/corpus/vgaps3}
BASE=https://samples.ffmpeg.org/V-codecs/CFHD
mkdir -p "$OUT/cfhd"
cd "$OUT/cfhd"
q="-nostdin -hide_banner -loglevel error -y"

[ -s timelapse-pt-momsapt.avi ] || curl -fsSL -o timelapse-pt-momsapt.avi "$BASE/timelapse-pt-momsapt.avi"
[ -s beartooth-head.avi ] || curl -fsSL -r 0-6000000 -o beartooth-head.avi "$BASE/MT_BeartoothHighway_1min_Cineform.avi"
[ -s lower_res.mov ] || curl -fsSL -o lower_res.mov "$BASE/lower_res.mov"

# Samples (packets in file order) and FFmpeg's decodes.
ffmpeg $q -i timelapse-pt-momsapt.avi -map 0:v -frames:v 2 -c copy -f image2 tl%02d.bin
ffmpeg $q -i timelapse-pt-momsapt.avi -map 0:v -frames:v 2 -f rawvideo -pix_fmt yuv422p10le tl.ref
ffmpeg $q -i beartooth-head.avi -map 0:v -frames:v 4 -c copy -f image2 bt%02d.bin
ffmpeg $q -i beartooth-head.avi -map 0:v -frames:v 4 -f rawvideo -pix_fmt yuv422p10le bt.ref
# The MOV's edit list drops its first sample; FFmpeg's first frame is sample 2.
ffmpeg $q -i lower_res.mov -map 0:v -frames:v 3 -c copy -f image2 lr%02d.bin
ffmpeg $q -i lower_res.mov -map 0:v -frames:v 1 -f rawvideo -pix_fmt bayer_rggb16le lr.ref
ls -l "$OUT/cfhd"
