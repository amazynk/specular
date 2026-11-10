#!/bin/sh
# ProRes frames from FFmpeg 8.0's encoders and FFmpeg's decodes as the oracle. Each NAME.frames
# is the samples one after another (`-c:v copy -f data`), NAME.yuv FFmpeg's planar decode
# (Y, Cb, Cr, then alpha). Run from this directory: sh gen_fixtures.sh
#   proxy, lt, std, hq   the 422 profiles (prores_ks), 10-bit
#   odd                  422, 90×50 (partial macroblocks and slices)
#   il-tff, il-bff       422 interlaced, both field orders
#   p4444, xq            4444 and 4444 XQ, 12-bit
#   alpha8, alpha16      4444 with 8- and 16-bit alpha
#   aw422, aw4444        prores_aw's frames (another encoder's choices)
set -e
q="-nostdin -loglevel error -y"
src="testsrc2=size=96x64:rate=25:duration=0.08"
make() {
    name=$1
    shift
    ffmpeg $q "$@" "$name.mov"
    ffmpeg $q -i "$name.mov" -map 0:v:0 -c:v copy -f data "$name.frames"
    pf=$(ffprobe -v error -select_streams v:0 -show_entries stream=pix_fmt -of csv=p=0 "$name.mov")
    ffmpeg $q -i "$name.mov" -map 0:v:0 -f rawvideo -pix_fmt "$pf" "$name.yuv"
    rm "$name.mov"
}
i="-f lavfi -i"
make proxy $i $src -c:v prores_ks -profile:v 0 -pix_fmt yuv422p10le
make lt $i $src -c:v prores_ks -profile:v 1 -pix_fmt yuv422p10le
make std $i $src -c:v prores_ks -profile:v 2 -pix_fmt yuv422p10le
make hq $i $src -c:v prores_ks -profile:v 3 -pix_fmt yuv422p10le
make odd $i testsrc2=size=90x50:rate=25:duration=0.08 -c:v prores_ks -profile:v 2 -pix_fmt yuv422p10le
make il-tff $i "$src,setfield=tff,interlace=scan=tff" -c:v prores_ks -profile:v 3 -flags +ildct -pix_fmt yuv422p10le
make il-bff $i "$src,setfield=bff,interlace=scan=bff" -c:v prores_ks -profile:v 3 -flags +ildct -pix_fmt yuv422p10le
make p4444 $i $src -c:v prores_ks -profile:v 4 -pix_fmt yuv444p10le
make xq $i $src -c:v prores_ks -profile:v 5 -pix_fmt yuv444p12le
make alpha8 $i "$src,format=yuva444p10le,geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='cr(X,Y)':a='Y*16'" -c:v prores_ks -profile:v 4 -alpha_bits 8 -pix_fmt yuva444p10le
make alpha16 $i "$src,format=yuva444p10le,geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='cr(X,Y)':a='X*10'" -c:v prores_ks -profile:v 4 -alpha_bits 16 -pix_fmt yuva444p10le
make aw422 $i testsrc2=size=90x50:rate=25:duration=0.08 -c:v prores_aw -profile:v 3 -pix_fmt yuv422p10le
make aw4444 $i $src -c:v prores_aw -profile:v 4 -pix_fmt yuv444p10le
