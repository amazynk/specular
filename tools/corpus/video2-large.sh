#!/usr/bin/env bash
# Larger Matroska / WebM / AVCHD clips for the timing test of crates/specular-video/tests/
# containers.rs (`timings_on_large_clips`), into tools/corpus/video2/large/ (gitignored, about
# 60 MB). Synthetic content (ffmpeg testsrc2 with noise, no third-party material), 4 s each:
#
#   vp9-1080p.webm     VP9 profile 0, 4 tile columns, alt-ref frames, Opus
#   vp9-2160p.webm     VP9 profile 0, 4K, 4 tile columns
#   vp9-10bit-1080p.webm  VP9 profile 2
#   vp8-1080p.webm     VP8, 4 token partitions, Vorbis
#   h264-1080p.mkv     H.264 High with B-frames, Opus
#   avchd-1080i.m2ts   H.264 MBAFF 1080i (25 frames/s, top field first), AC-3, 192-byte packets
#
# Usage: tools/corpus/video2-large.sh [--clean]. Needs ffmpeg with libvpx, libx264, libopus.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$ROOT/tools/corpus/video2/large"
for a in "$@"; do
  case $a in
    --clean) rm -rf "$OUT"; echo "removed $OUT"; exit 0 ;;
    *) echo "usage: $0 [--clean]" >&2; exit 2 ;;
  esac
done
command -v ffmpeg >/dev/null || { echo "video2-large: ffmpeg not found" >&2; exit 1; }
gb=$(df -Pk "$ROOT" | awk 'NR==2{print int($4/1048576)}')
[ "$gb" -ge 8 ] || { echo "video2-large: only ${gb} GB free" >&2; exit 1; }
mkdir -p "$OUT"
cd "$OUT"
FF=(nice ffmpeg -hide_banner -loglevel error -y)
src() { echo "testsrc2=s=$1:r=$2:d=4,noise=alls=12:allf=t"; }
SINE="sine=frequency=440:sample_rate=48000:duration=4"

[ -f vp9-1080p.webm ] || "${FF[@]}" -f lavfi -i "$(src 1920x1080 30)" -f lavfi -i "$SINE" \
  -c:v libvpx-vp9 -pix_fmt yuv420p -b:v 6M -g 60 -tile-columns 2 -row-mt 1 -deadline good \
  -cpu-used 5 -c:a libopus -shortest vp9-1080p.webm
[ -f vp9-2160p.webm ] || "${FF[@]}" -f lavfi -i "$(src 3840x2160 30)" \
  -c:v libvpx-vp9 -pix_fmt yuv420p -b:v 16M -g 60 -tile-columns 2 -row-mt 1 -deadline realtime \
  -cpu-used 8 vp9-2160p.webm
[ -f vp9-10bit-1080p.webm ] || "${FF[@]}" -f lavfi -i "$(src 1920x1080 30)" \
  -c:v libvpx-vp9 -pix_fmt yuv420p10le -profile:v 2 -b:v 6M -g 60 -tile-columns 2 -row-mt 1 \
  -deadline realtime -cpu-used 8 vp9-10bit-1080p.webm
[ -f vp8-1080p.webm ] || "${FF[@]}" -f lavfi -i "$(src 1920x1080 30)" -f lavfi -i "$SINE" \
  -c:v libvpx -pix_fmt yuv420p -b:v 6M -g 60 -slices 4 -deadline realtime -cpu-used 8 \
  -c:a libvorbis -shortest vp8-1080p.webm
[ -f h264-1080p.mkv ] || "${FF[@]}" -f lavfi -i "$(src 1920x1080 30)" -f lavfi -i "$SINE" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -b:v 8M -g 60 -bf 3 -c:a libopus -shortest \
  h264-1080p.mkv
[ -f avchd-1080i.m2ts ] || "${FF[@]}" -f lavfi -i "$(src 1920x1080 50)" -f lavfi -i "$SINE" \
  -vf "tinterlace=interleave_top,setfield=tff" -c:v libx264 -preset veryfast -pix_fmt yuv420p \
  -flags +ilme+ildct -x264-params tff=1:keyint=12:bframes=2 -b:v 15M -c:a ac3 -shortest \
  -f mpegts -mpegts_m2ts_mode 1 avchd-1080i.m2ts
ls -l "$OUT"
