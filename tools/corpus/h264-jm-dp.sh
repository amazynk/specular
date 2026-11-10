#!/usr/bin/env bash
# Builds the JM H.264 reference software (test oracle only: never committed, never linked
# into Specular) and makes Extended-profile streams with it: data partitioning (with and
# without slices, constrained intra, B frames, fields, weighted prediction) and SP/SI.
# JM's own decoder writes each stream's oracle YUV, reduced to per-frame MD5s.
#
#   $JM_DIR                      JM source (default $CARGO_TARGET_DIR/jm/JM-<commit>)
#   $OUT/<name>.264              the stream
#   $OUT/<name>.framemd5         "0, n, n, 1, size, md5" per frame of ldecod's output
#   $OUT/manifest.tsv            name, file, width, height, pix_fmt, field_order, frames
#
# OUT defaults to $SPECULAR_H264DP_CORPUS, else $CARGO_TARGET_DIR/corpus/h264dp. About 5 MB.
# Then: SPECULAR_H264_CORPUS=$OUT cargo test -p specular-h264 --test conformance
# Needs curl, cmake, a C compiler and ffmpeg (for the synthetic input pictures only).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TD=${CARGO_TARGET_DIR:-$ROOT/target}
JM_COMMIT=8b34eee1576952dc2a04cd2fdb52febfde4030b2
# SHA-256 of the uncompressed tar (`git archive` of JM_COMMIT; its pax header names the
# commit). The server gzips the archive anew now and then, so the .tar.gz bytes change while
# the tar inside stays the same: the tar is what is pinned.
JM_TAR_SHA=25413af1b9b72339cd313fe5982341dcddf032be825f535af53283b1385b6de5
JM_DIR=${JM_DIR:-$TD/jm/JM-$JM_COMMIT}
OUT=${SPECULAR_H264DP_CORPUS:-$TD/corpus/h264dp}
MIN_FREE_GB=${MIN_FREE_GB:-8}
for t in curl gzip cmake make cc ffmpeg md5sum sha256sum; do
  command -v $t >/dev/null || { echo "h264-jm-dp: $t not found" >&2; exit 1; }
done
gb=$(df -Pk "$TD" | awk 'NR==2{print int($4/1048576)}')
[ "$gb" -ge "$MIN_FREE_GB" ] || { echo "h264-jm-dp: only ${gb} GB free" >&2; exit 1; }

if [ ! -x "$JM_DIR/bin/lencod_static" ]; then
  mkdir -p "$(dirname "$JM_DIR")"
  tgz="$(dirname "$JM_DIR")/JM-$JM_COMMIT.tar.gz"
  [ -s "$tgz" ] || curl -fsSL --retry 3 -o "$tgz" \
    "https://vcgit.hhi.fraunhofer.de/jvet/JM/-/archive/$JM_COMMIT/JM-$JM_COMMIT.tar.gz"
  sha=$(gzip -dc "$tgz" | sha256sum | cut -d' ' -f1)
  [ "$sha" = "$JM_TAR_SHA" ] || { echo "h264-jm-dp: JM tarball hash mismatch ($sha)" >&2; exit 1; }
  tar -xzf "$tgz" -C "$(dirname "$JM_DIR")"
  cmake -S "$JM_DIR" -B "$JM_DIR/build" -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_C_FLAGS="-O2 -fcommon -w" > "$JM_DIR/cmake.log"
  nice make -C "$JM_DIR/build" -j2 > "$JM_DIR/make.log"
fi
ENC=$JM_DIR/bin/lencod_static
DEC=$JM_DIR/bin/ldecod_static

mkdir -p "$OUT/work"
W=$OUT/work
# Synthetic moving input (QCIF and CIF, 4:2:0).
[ -s "$W/qcif.yuv" ] || ffmpeg -nostdin -loglevel error -y -f lavfi \
  -i testsrc2=size=176x144:rate=30:duration=1 -pix_fmt yuv420p -f rawvideo "$W/qcif.yuv"
[ -s "$W/cif.yuv" ] || ffmpeg -nostdin -loglevel error -y -f lavfi \
  -i "mandelbrot=size=352x288:rate=30,trim=duration=0.4" -pix_fmt yuv420p -f rawvideo "$W/cif.yuv"

# make NAME INPUT WIDTH HEIGHT FRAMES FIELD_ORDER [JM -p overrides...]
make_stream() {
  local name=$1 input=$2 w=$3 h=$4 frames=$5 order=$6
  shift 6
  local args=()
  for p in "$@"; do args+=(-p "$p"); done
  (cd "$W" && "$ENC" -d "$JM_DIR/cfg/encoder_extended.cfg" \
    -p InputFile="$input" -p SourceWidth="$w" -p SourceHeight="$h" \
    -p FramesToBeEncoded="$frames" -p OutputFile="$name.264" -p ReconFile=rec.yuv \
    -p StatsFile=stats.dat -p ProfileIDC=88 -p LevelIDC=30 -p SymbolMode=0 \
    "${args[@]}" > "$name.enc.log" 2>&1) || { tail -5 "$W/$name.enc.log" >&2; exit 1; }
  (cd "$W" && "$DEC" -i "$name.264" -o "$name.yuv" > "$name.dec.log" 2>&1) \
    || { tail -5 "$W/$name.dec.log" >&2; exit 1; }
  cp "$W/$name.264" "$OUT/$name.264"
  local size=$((w * h * 3 / 2)) n=0 total
  total=$(( $(stat -c %s "$W/$name.yuv") / size ))
  : > "$OUT/$name.framemd5"
  while [ $n -lt "$total" ]; do
    md=$(dd if="$W/$name.yuv" bs=$size skip=$n count=1 status=none | md5sum | cut -d' ' -f1)
    echo "0, $n, $n, 1, $size, $md" >> "$OUT/$name.framemd5"
    n=$((n + 1))
  done
  printf '%s\t%s\t%s\t%s\tyuv420p\t%s\t%s\n' "$name" "$name.264" "$w" "$h" "$order" "$total" \
    >> "$OUT/manifest.tsv.part"
  rm -f "$W/$name.yuv"
}

: > "$OUT/manifest.tsv.part"
make_stream jmdp_ipp qcif.yuv 176 144 30 progressive PartitionMode=1 NumberBFrames=0
make_stream jmdp_ibp qcif.yuv 176 144 30 progressive PartitionMode=1 NumberBFrames=1
make_stream jmdp_slices qcif.yuv 176 144 30 progressive PartitionMode=1 NumberBFrames=1 \
  SliceMode=1 SliceArgument=17
make_stream jmdp_cip qcif.yuv 176 144 30 progressive PartitionMode=1 NumberBFrames=1 \
  UseConstrainedIntraPred=1 IntraPeriod=10
make_stream jmdp_cip_refresh qcif.yuv 176 144 30 progressive PartitionMode=1 \
  NumberBFrames=1 UseConstrainedIntraPred=1 RandomIntraMBRefresh=40
make_stream jmdp_refresh qcif.yuv 176 144 30 progressive PartitionMode=1 \
  NumberBFrames=0 RandomIntraMBRefresh=30 SliceMode=1 SliceArgument=33
make_stream jmdp_wp qcif.yuv 176 144 20 progressive PartitionMode=1 NumberBFrames=2 \
  WeightedPrediction=1 WeightedBiprediction=1
make_stream jmdp_cif cif.yuv 352 288 12 progressive PartitionMode=1 NumberBFrames=1 \
  SliceMode=2 SliceArgument=600
make_stream jmdp_field cif.yuv 352 288 12 tt PartitionMode=1 NumberBFrames=1 PicInterlace=1
make_stream jmdp_paff cif.yuv 352 288 12 tt PartitionMode=1 NumberBFrames=1 PicInterlace=2
# (MBAFF is left out: this JM's encoder stops with signal 16 on MbInterlace=1.)
make_stream jmsp_periodic qcif.yuv 176 144 30 progressive PartitionMode=0 NumberBFrames=0 \
  SPPicturePeriodicity=4
mv "$OUT/manifest.tsv.part" "$OUT/manifest.tsv"
echo "h264-jm-dp: $(wc -l < "$OUT/manifest.tsv") streams in $OUT"
