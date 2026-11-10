#!/bin/sh
# Old-codec streams from FFmpeg 8.0's encoders, FFmpeg's decodes as the oracle (a tool only;
# nothing of it is linked). NAME.frames: u32 width, height, depth, then per frame u32 size and
# the packet (big-endian; packets split by FFmpeg's image2 muxer). NAME.out: FFmpeg's decode in
# the codec's own format (palette frames carry their 1024-byte palette after the indices).
# Run from this directory: sh gen_fixtures.sh
set -e
q="-nostdin -loglevel error -y"
make() {
    name=$1 ext=$2 depth=$3 out=$4 size=$5
    shift 5
    w=${size%x*}
    h=${size#*x}
    ffmpeg $q -f lavfi -i "testsrc2=size=$size:rate=25:duration=0.2,noise=alls=8:allf=t" "$@" "$name.$ext"
    rm -rf "$name.d"
    mkdir "$name.d"
    ffmpeg $q -i "$name.$ext" -map 0:v:0 -c copy -f image2 "$name.d/%04d.bin"
    python3 -I pack.py "$name.frames" "$w" "$h" "$depth" "$name.d"
    ffmpeg $q -i "$name.$ext" -map 0:v:0 -f rawvideo -pix_fmt "$out" "$name.out"
    rm -rf "$name.d" "$name.$ext"
}
make qtrle24 mov 24 rgb24 64x48 -c:v qtrle -pix_fmt rgb24 -g 3
make qtrle16 mov 16 rgb555le 70x46 -c:v qtrle -pix_fmt rgb555be -g 3
make qtrle32 mov 32 argb 64x48 -c:v qtrle -pix_fmt argb -g 3
make qtrle8 mov 40 pal8 68x46 -c:v qtrle -pix_fmt gray -g 3
make smc mov 8 pal8 64x48 -c:v smc -pix_fmt pal8 -g 3
make smc-odd mov 8 pal8 70x46 -c:v smc -pix_fmt pal8 -g 3
make rpza mov 16 rgb555le 64x48 -c:v rpza -pix_fmt rgb555be -g 3
make rpza-odd mov 16 rgb555le 70x46 -c:v rpza -pix_fmt rgb555be -g 3
make cinepak avi 24 rgb24 64x48 -c:v cinepak -pix_fmt rgb24 -g 3
make cinepak-grey avi 24 rgb24 64x48 -c:v cinepak -pix_fmt gray -g 3
make cinepak-strips avi 24 rgb24 96x80 -c:v cinepak -pix_fmt rgb24 -max_strips 3 -min_strips 3 -g 3
make msvideo1 avi 16 rgb555le 64x48 -c:v msvideo1 -pix_fmt rgb555le -g 3
make msrle avi 8 pal8 70x46 -c:v msrle -pix_fmt pal8 -g 3
make flashsv flv 24 rgb24 72x50 -c:v flashsv -g 3
make flashsv2 flv 24 rgb24 72x50 -c:v flashsv2 -g 3
make flashsv2-big flv 24 rgb24 160x120 -c:v flashsv2 -g 4
