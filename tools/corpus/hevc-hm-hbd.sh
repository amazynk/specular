#!/usr/bin/env bash
# HEVC inter pictures deeper than 12 bits (RExt 14- and 16-bit, extended precision): no
# conformance bitstream has them (the 16-bit ones are all intra) and libavcodec stops at 12
# bits, so the HM reference software (test oracle only: never committed, never linked into
# Specular) encodes synthetic clips and its decoder gives the oracle.
#
#   $OUT/<name>.bit        the stream (random access with B pictures, or low delay P with
#                          weighted prediction)
#   $OUT/<name>.framemd5   "0, n, n, 1, size, md5" per frame of HM's output (display order,
#                          little-endian 16-bit samples)
#   $OUT/<name>.yuv.md5    MD5 of the whole output
#   $OUT/manifest.tsv      name, file, width, height, pix_fmt, frames
#
# OUT defaults to $CARGO_TARGET_DIR/corpus/hevc-hbd (under 2 MB). Then:
#   SPECULAR_HEVC_CORPUS=$OUT cargo test -p specular-hevc --test conformance conformance
# Needs curl, cmake, make, a C++ compiler, python3 and ffmpeg (for the input pictures only).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TD=${CARGO_TARGET_DIR:-$ROOT/target}
HM_TAG=HM-18.0
HM_DIR=${HM_DIR:-$TD/hm/HM-$HM_TAG}
OUT=${SPECULAR_HEVC_HBD_CORPUS:-$TD/corpus/hevc-hbd}
for t in curl cmake make c++ ffmpeg python3; do
  command -v $t >/dev/null || { echo "hevc-hm-hbd: $t not found" >&2; exit 1; }
done
BIN=$HM_DIR/bin/umake
if ! ls "$BIN"/*/*/release/TAppDecoder >/dev/null 2>&1; then
  mkdir -p "$(dirname "$HM_DIR")"
  tgz="$(dirname "$HM_DIR")/$HM_TAG.tar.gz"
  [ -s "$tgz" ] || curl -fsSL --retry 3 -o "$tgz" \
    "https://vcgit.hhi.fraunhofer.de/jvet/HM/-/archive/$HM_TAG/HM-$HM_TAG.tar.gz"
  tar -xzf "$tgz" -C "$(dirname "$HM_DIR")"
  # HIGH_BITDEPTH: 32-bit samples inside, for 16-bit video.
  cmake -S "$HM_DIR" -B "$HM_DIR/build" -DCMAKE_BUILD_TYPE=Release -DHIGH_BITDEPTH=ON \
    -DCMAKE_CXX_FLAGS="-Wno-error -w" > "$HM_DIR/cmake.log"
  nice make -C "$HM_DIR/build" -j2 TAppEncoder TAppDecoder > "$HM_DIR/make.log"
fi
ENC=$(ls "$BIN"/*/*/release/TAppEncoder | head -1)
DEC=$(ls "$BIN"/*/*/release/TAppDecoder | head -1)
CFG=$HM_DIR/cfg
mkdir -p "$OUT/work"
W=$OUT/work
: > "$OUT/manifest.tsv"
FADE=

# make NAME FMT(400|420|444) DEPTH CONFIG FRAMES [HM options...]
make_stream() {
  local name=$1 fmt=$2 depth=$3 cfg=$4 frames=$5
  shift 5
  local pix
  case $fmt in
    400) pix=gray16le ;;
    420) pix=yuv420p16le ;;
    444) pix=yuv444p16le ;;
  esac
  # The manifest names the stream's own depth.
  local label=${pix/16le/${depth}le}
  ffmpeg -nostdin -loglevel error -y -f lavfi \
    -i "testsrc2=size=96x64:rate=25,trim=end_frame=$frames$FADE" -pix_fmt "$pix" \
    -f rawvideo "$W/in.yuv"
  (cd "$W" && "$ENC" -c "$CFG/$cfg" -c "$CFG/hbd/hbd.cfg" -i in.yuv -wdt 96 -hgt 64 -fr 25 \
    -f "$frames" --InputBitDepth=16 --InternalBitDepth="$depth" --OutputBitDepth="$depth" \
    --InputChromaFormat="$fmt" --ChromaFormatIDC="$fmt" --Profile=main-RExt --Level=6.2 \
    --SEIDecodedPictureHash=1 -b "$name.bit" -o /dev/null "$@" > "$name.enc.log")
  (cd "$W" && "$DEC" -b "$name.bit" -o "$name.yuv" -d 0 > "$name.dec.log")
  mv "$W/$name.bit" "$OUT/$name.bit"
  python3 -I - "$W/$name.yuv" 96 64 "$fmt" > "$OUT/$name.framemd5" <<'PY'
import hashlib, sys
path, w, h, fmt = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
c = {"400": 0, "420": (w // 2) * (h // 2), "444": w * h}[fmt]
size = (w * h + 2 * c) * 2
with open(path, "rb") as f:
    n = 0
    while (b := f.read(size)) and len(b) == size:
        print(f"0, {n:10}, {n:10}, {1:8}, {size:8}, {hashlib.md5(b).hexdigest()}")
        n += 1
PY
  md5sum < "$W/$name.yuv" | cut -c1-32 > "$OUT/$name.yuv.md5"
  printf '%s\t%s\t96\t64\t%s\t%s\n' "$name" "$name.bit" "$label" \
    "$(grep -c . "$OUT/$name.framemd5")" >> "$OUT/manifest.tsv"
  rm -f "$W/$name.yuv"
}

make_stream HBD16_400_RA 400 16 encoder_randomaccess_main_rext.cfg 17 --IntraPeriod=16
make_stream HBD16_420_RA 420 16 encoder_randomaccess_main_rext.cfg 17 --IntraPeriod=16
make_stream HBD16_444_RA 444 16 encoder_randomaccess_main_rext.cfg 17 --IntraPeriod=16
make_stream HBD14_444_RA 444 14 encoder_randomaccess_main_rext.cfg 17 --IntraPeriod=16
# A fade, so that the encoder picks weights other than the default ones.
FADE=,fade=in:0:6
make_stream HBD16_444_LDP_WP 444 16 encoder_lowdelay_P_main.cfg 6 --WeightedPredP=1 \
  --IntraPeriod=-1
# (HM 18's decoder aborts on its own weighted B pictures at 16 bits: low delay B without.)
make_stream HBD16_420_LDB 420 16 encoder_lowdelay_main_rext.cfg 6 --IntraPeriod=-1
FADE=
make_stream HBD16_444_RA_NOEXT 444 16 encoder_randomaccess_main_rext.cfg 17 --IntraPeriod=16 \
  --ExtendedPrecision=0 --HighPrecisionPredictionWeighting=0
rm -rf "$W"
echo "hevc-hm-hbd: $(wc -l < "$OUT/manifest.tsv") streams in $OUT"
