#!/bin/sh
# HAP fixtures. HAP, HAP Alpha and HAP Q come from FFmpeg 8.0's encoder (none / Snappy, one
# or four chunks), with FFmpeg's RGBA decode as the oracle (a tool only; nothing of it is
# linked). FFmpeg encodes multiples of 4 only, so the odd-size fixtures are 72×48 streams
# declared 70×46 (the texture covers the picture rounded up to whole blocks) and the oracle is
# cropped. HAP Q Alpha, HAP Alpha-Only and HAP R are written by hapwrite.py (own BC4 and BC7
# encoders whose values decode exactly; see there).
# NAME.frames: u32 width, height, 0, then per frame u32 size and the frame (big-endian).
# NAME.out: the expected RGBA of every frame.
# Run from this directory: sh gen_fixtures.sh
set -e
q="-nostdin -loglevel error -y"
src="testsrc2=size=72x48:rate=25:duration=0.08,format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='mod(X*5+Y*3,256)'"
make() {
    name=$1 w=$2 h=$3
    shift 3
    ffmpeg $q -f lavfi -i "$src" -c:v hap "$@" "$name.mov"
    rm -rf "$name.d"
    mkdir "$name.d"
    ffmpeg $q -i "$name.mov" -map 0:v:0 -c copy -f image2 "$name.d/%04d.bin"
    python3 -I pack.py "$name.frames" "$w" "$h" 0 "$name.d"
    ffmpeg $q -i "$name.mov" -map 0:v:0 -vf "crop=$w:$h:0:0" -f rawvideo -pix_fmt rgba "$name.out"
    rm -rf "$name.d"
}
make hap 72 48 -format hap -compressor snappy -chunks 1
make hap-none-odd 70 46 -format hap -compressor none
make hapa-chunks 70 46 -format hap_alpha -compressor snappy -chunks 4
make hapa-none 72 48 -format hap_alpha -compressor none -chunks 1
make hapq 72 48 -format hap_q -compressor snappy -chunks 4
make hapq-none-odd 70 46 -format hap_q -compressor none
# HAP Q Alpha wraps the stored HAP Q textures (and FFmpeg's RGB) with own BC4 alpha.
python3 -I hapwrite.py hapm hapq-none-odd.frames hapq-none-odd.out 70 46
python3 -I hapwrite.py hapalpha-only - - 70 46
python3 -I hapwrite.py hapr - - 70 46
rm -f ./*.mov
