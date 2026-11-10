#!/bin/sh
# Generates the small VP8 fixtures in crates/formats/specular-vp8/tests/data/ from ffmpeg's
# testsrc2 with libvpx, each with the per-frame MD5s of ffmpeg's own VP8 decoder
# (`-f framemd5`, shown frames only). Also writes a 1080p and a 4K clip for the benchmark
# into tools/corpus/video2/specular-vp8/ (gitignored).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/../../crates/formats/specular-vp8/tests/data"
big="$here/video2/specular-vp8"
mkdir -p "$out" "$big"

enc() { # name size frames args...
    name=$1 size=$2 n=$3
    shift 3
    ffmpeg -nostdin -loglevel error -y -f lavfi -i "testsrc2=size=$size:rate=25" \
        -frames:v "$n" -pix_fmt yuv420p -c:v libvpx "$@" "$out/$name.ivf"
    ffmpeg -nostdin -loglevel error -y -c:v vp8 -i "$out/$name.ivf" -f framemd5 \
        "$out/$name.framemd5"
}

enc2() { # two-pass (alt-ref frames need it): name size frames args...
    name=$1 size=$2 n=$3
    shift 3
    for pass in 1 2; do
        ffmpeg -nostdin -loglevel error -y -f lavfi -i "testsrc2=size=$size:rate=25" \
            -frames:v "$n" -pix_fmt yuv420p -c:v libvpx -pass $pass \
            -passlogfile "$big/passlog" "$@" -f ivf "$out/$name.ivf"
    done
    ffmpeg -nostdin -loglevel error -y -c:v vp8 -i "$out/$name.ivf" -f framemd5 \
        "$out/$name.framemd5"
}

# 177x143 (libvpx via ffmpeg codes 176x142: an odd macroblock row), four token partitions.
enc odd-partitions 177x143 20 -b:v 300k -slices 4 -auto-alt-ref 0
# Hidden alt-ref frames (show_frame 0), golden/altref references.
enc2 altref 160x120 40 -b:v 150k -auto-alt-ref 1 -lag-in-frames 16 -arnr-maxframes 5
# Error resilient, eight partitions.
enc resilient 144x96 20 -b:v 150k -error-resilient 1 -slices 8 -auto-alt-ref 0
# Low bitrate, realtime: cyclic refresh segmentation, static threshold, skips.
enc lowrate 320x240 40 -b:v 30k -deadline realtime -cpu-used 8 -static-thresh 1000 \
    -error-resilient 1 -auto-alt-ref 0
# Bitstream versions 1-3 (bilinear prediction, simple loop filter, full-pixel chroma).
enc profile1 96x80 16 -b:v 100k -profile:v 1 -auto-alt-ref 0
enc profile2 96x80 16 -b:v 100k -profile:v 2 -auto-alt-ref 0
enc profile3 96x80 16 -b:v 100k -profile:v 3 -auto-alt-ref 0
# Very high quality: large coefficients, every token category.
enc highq 64x64 6 -qmin 0 -qmax 4 -b:v 5M -auto-alt-ref 0

# Benchmark clips.
ffmpeg -nostdin -loglevel error -y -f lavfi -i "testsrc2=size=1920x1080:rate=30" \
    -frames:v 120 -pix_fmt yuv420p -c:v libvpx -b:v 6M -slices 4 -threads 4 -auto-alt-ref 1 \
    -lag-in-frames 16 "$big/bench-1080p.ivf"
ffmpeg -nostdin -loglevel error -y -f lavfi -i "testsrc2=size=3840x2160:rate=30" \
    -frames:v 60 -pix_fmt yuv420p -c:v libvpx -b:v 16M -slices 8 -threads 4 -auto-alt-ref 1 \
    -lag-in-frames 16 "$big/bench-2160p.ivf"
rm -f "$big"/passlog*
du -sh "$out"
