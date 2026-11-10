#!/bin/sh
# FFV1 streams from FFmpeg 8.0's encoder and FFmpeg's decodes as the oracle (a tool only;
# nothing of it is linked). NAME.frames is the stream cut out of an AVI by avi_frames.py
# (size, configuration record, frames); NAME.yuv FFmpeg's planar decode (Y, Cb, Cr or G, B, R,
# then alpha). Run from this directory: sh gen_fixtures.sh
#   v0-rice, v0-range          version 0, Golomb-Rice and range coder, 8-bit 4:2:0
#   v1-tab10, v1-rgb           version 1, custom state table 10-bit 4:2:2; Golomb RGB
#   v1-16                      version 1, 16-bit 4:4:4 (the signed predictor of RFC 9043 §3.3.1)
#   v3-*                       version 3: slices, CRCs, large contexts, alpha, RGB 8-16 bits,
#                              greyscale, 4:1:1, 4:4:0, odd sizes, frames that are not keyframes
set -e
q="-nostdin -loglevel error -y"
src="testsrc2=size=80x48:rate=25:duration=0.12"
odd="testsrc2=size=90x50:rate=25:duration=0.08"
make() {
    name=$1
    out=$2
    shift 2
    ffmpeg $q "$@" "$name.avi"
    python3 -I avi_frames.py "$name.avi" "$name.frames"
    ffmpeg $q -i "$name.avi" -map 0:v:0 -f rawvideo -pix_fmt "$out" "$name.yuv"
    rm "$name.avi"
}
i="-f lavfi -i"
make v0-rice yuv420p $i $src -c:v ffv1 -level 0 -coder rice -g 2 -pix_fmt yuv420p
make v0-range yuv420p $i $src -c:v ffv1 -level 0 -coder range_def -g 2 -pix_fmt yuv420p
make v1-tab10 yuv422p10le $i $src -c:v ffv1 -level 1 -coder range_tab -context 1 -pix_fmt yuv422p10le
make v1-rgb gbrp $i $src -c:v ffv1 -level 1 -coder rice -pix_fmt bgr0
make v1-16 yuv444p16le $i $src -c:v ffv1 -level 1 -coder range_tab -pix_fmt yuv444p16le
make v3-420 yuv420p $i $src -c:v ffv1 -level 3 -slices 4 -slicecrc 1 -g 1 -pix_fmt yuv420p
make v3-rice yuv420p $i $src -c:v ffv1 -level 3 -coder rice -slices 6 -slicecrc 1 -g 2 -pix_fmt yuv420p
make v3-ctx yuv444p $i $src -c:v ffv1 -level 3 -coder range_tab -context 1 -slices 9 -g 2 -pix_fmt yuv444p
make v3-alpha yuva420p $i "$src,format=yuva420p,geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='cr(X,Y)':a='X*2+Y'" -c:v ffv1 -level 3 -slices 4 -pix_fmt yuva420p
make v3-rgb gbrp $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt bgr0
make v3-rgba gbrap $i "$src,format=bgra,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='X+Y*2'" -c:v ffv1 -level 3 -coder rice -slices 4 -pix_fmt bgra
make v3-rgb10 gbrp10le $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt gbrp10le
make v3-rgba12 gbrap12le $i "$src,format=gbrap12le,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='X*40'" -c:v ffv1 -level 3 -slices 4 -pix_fmt gbrap12le
make v3-rgb16 gbrp16le $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt gbrp16le
make v3-gray gray $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt gray
make v3-gray16 gray16le $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt gray16le
make v3-411 yuv411p $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt yuv411p
make v3-440 yuv440p $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt yuv440p
make v3-12 yuv420p12le $i $src -c:v ffv1 -level 3 -slices 4 -slicecrc 1 -pix_fmt yuv420p12le
make v3-16 yuv422p16le $i $src -c:v ffv1 -level 3 -slices 4 -pix_fmt yuv422p16le
make v3-odd yuv420p $i $odd -c:v ffv1 -level 3 -slices 6 -g 2 -pix_fmt yuv420p
make v3-odd-rice yuv422p10le $i $odd -c:v ffv1 -level 3 -coder rice -slices 4 -pix_fmt yuv422p10le
