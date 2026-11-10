#!/bin/sh
# FFV1, uncompressed video and the old codecs in AVI, QuickTime and Matroska, from FFmpeg 8.0's
# encoders; FFmpeg's decodes (NAME.yuv, planar in the pixel format given) are the oracle (a
# tool only; nothing of it is linked). Run from this directory: sh gen_fixtures_vgaps.sh
set -e
q="-nostdin -loglevel error -y"
src="testsrc2=size=32x24:rate=25:duration=0.12"
make() {
    name=$1 out=$2
    shift 2
    ffmpeg $q -f lavfi -i "$src" "$@" "vg-$name"
    ffmpeg $q -i "vg-$name" -map 0:v:0 -f rawvideo -pix_fmt "$out" "vg-${name%.*}.yuv"
}
# FFV1
make ffv1-420.mkv yuv420p -c:v ffv1 -level 3 -slices 4 -pix_fmt yuv420p
make ffv1-v1.avi yuv422p10le -c:v ffv1 -level 1 -coder range_tab -pix_fmt yuv422p10le
make ffv1-rgb.mov gbrp -c:v ffv1 -level 3 -pix_fmt bgr0
make ffv1-gray.mkv gray -c:v ffv1 -level 3 -pix_fmt gray
make ffv1-alpha.mkv yuva420p -vf "format=yuva420p,geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='cr(X,Y)':a='X*8'" -c:v ffv1 -level 3 -pix_fmt yuva420p
# Uncompressed
make raw-i420.avi yuv420p -c:v rawvideo -pix_fmt yuv420p
make raw-bgr24.avi gbrp -c:v rawvideo -pix_fmt bgr24
make raw-pal8.avi gbrp -c:v rawvideo -pix_fmt pal8
make raw-555.avi gbrp -c:v rawvideo -pix_fmt rgb555le
make raw-nv12.avi yuv420p -c:v rawvideo -pix_fmt nv12
make raw-yuy2.avi yuv422p -c:v rawvideo -pix_fmt yuyv422
make raw-y41p.avi yuv411p -c:v y41p
make raw-yuv4.avi yuv420p -c:v yuv4
make raw-v410.avi yuv444p10le -c:v v410
make raw-r10k.avi gbrp10le -c:v r10k
make raw-v210.mov yuv422p10le -c:v v210
make raw-v308.mov yuv444p -c:v v308
make raw-r210.mov gbrp10le -c:v r210
make raw-2vuy.mov yuv422p -c:v rawvideo -pix_fmt uyvy422
make raw-yuvs.mov yuv422p -c:v rawvideo -pix_fmt yuyv422
make raw-rgb24.mov gbrp -c:v rawvideo -pix_fmt rgb24
make raw-pal8.mov gbrp -c:v rawvideo -pix_fmt pal8
make raw-b48r.mov gbrp16le -c:v rawvideo -pix_fmt rgb48be
make raw-argb.mov gbrap -c:v rawvideo -pix_fmt argb
make raw-i420.mkv yuv420p -c:v rawvideo -pix_fmt yuv420p
make raw-uyvy.mkv yuv422p -c:v rawvideo -pix_fmt uyvy422
make raw-y800.mkv gbrp -c:v rawvideo -pix_fmt gray
make raw-y42b.mkv yuv422p -c:v rawvideo -pix_fmt yuv422p
# Old codecs
make qtrle24.mov gbrp -c:v qtrle -pix_fmt rgb24 -g 2
make qtrle-gray.mov gbrp -c:v qtrle -pix_fmt gray -g 2
make qtrle-argb.mov gbrap -c:v qtrle -pix_fmt argb -g 2
make smc.mov gbrp -c:v smc -pix_fmt pal8 -g 2
make rpza.mov gbrp -c:v rpza -pix_fmt rgb555be -g 2
make cinepak.avi gbrp -c:v cinepak -pix_fmt rgb24 -g 2
make cinepak.mov gbrp -c:v cinepak -pix_fmt rgb24 -g 2
make msvideo1.avi gbrp -c:v msvideo1 -pix_fmt rgb555le -g 2
make msrle.avi gbrp -c:v msrle -pix_fmt pal8 -g 2
