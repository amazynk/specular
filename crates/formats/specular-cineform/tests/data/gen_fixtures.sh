#!/bin/sh
# CineForm streams from FFmpeg 8.0's cfhd encoder, FFmpeg's decode as the oracle (a tool only;
# nothing of it is linked). NAME.frames: u32 width, height, 0, then per sample u32 size and the
# sample (big-endian). NAME.out: FFmpeg's decode (yuv422p10le, gbrp12le or gbrap12le).
# Run from this directory: sh gen_fixtures.sh
set -e
q="-nostdin -loglevel error -y"
make() {
    name=$1 size=$2 pix=$3 src=$4
    shift 4
    w=${size%x*}
    h=${size#*x}
    ffmpeg $q -f lavfi -i "$src" -frames:v 2 -s "$size" -c:v cfhd -pix_fmt "$pix" "$@" "$name.mov"
    rm -rf "$name.d"
    mkdir "$name.d"
    ffmpeg $q -i "$name.mov" -map 0:v:0 -c copy -f image2 "$name.d/%04d.bin"
    python3 -I pack.py "$name.frames" "$w" "$h" 0 "$name.d"
    ffmpeg $q -i "$name.mov" -map 0:v:0 -f rawvideo -pix_fmt "$pix" "$name.out"
    rm -rf "$name.d" "$name.mov"
}
noise="testsrc2=size=64x48:rate=25,noise=alls=12:allf=t"
make cf-422 64x48 yuv422p10le "$noise"
make cf-422-low 80x46 yuv422p10le "$noise" -quality low
make cf-rgb 64x48 gbrp12le "$noise"
make cf-rgb-mid 48x40 gbrp12le "testsrc2=size=48x40:rate=25" -quality medium
# Alpha: a ramp across the picture.
make cf-rgba 64x48 gbrap12le "testsrc2=size=64x48:rate=25,format=gbrap,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='X*4'"
