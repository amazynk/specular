#!/bin/sh
# CineForm, HAP, VC-2 / Dirac, Flash Screen Video and OGM (video and sound) fixtures. FFmpeg 8.0's encoders and
# muxers where it has them, the own Ogg writer (oggwrite.py) for OGM and Dirac in Ogg; FFmpeg's
# decodes (NAME.yuv, planar in the pixel format given) are the oracle (a tool only; nothing of
# it is linked). VC-2 stays at transform depth 3: deeper at 64×48, FFmpeg 8.0 is off by a few
# steps where the smallest bands are one or two samples wide (see specular-vc2's fixtures).
# Run from this directory: sh gen_fixtures_vgaps3.sh
set -e
q="-nostdin -loglevel error -y"
src="testsrc2=size=64x48:rate=25:duration=0.12"
make() {
    name=$1 out=$2
    shift 2
    ffmpeg $q -f lavfi -i "$src" "$@" "vg3-$name"
    ffmpeg $q -i "vg3-$name" -map 0:v:0 -f rawvideo -pix_fmt "$out" "vg3-${name%.*}.yuv"
}
make cfhd.mov yuv422p10le -c:v cfhd -pix_fmt yuv422p10le
make cfhd-rgba.avi gbrap12le -vf "format=gbrap,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='X*4'" -c:v cfhd -pix_fmt gbrap12le
make hap.mov gbrp -c:v hap -format hap
make hapq.mov gbrp -c:v hap -format hap_q -chunks 2
make hapa.mov gbrap -vf "format=rgba,geq=r='r(X,Y)':g='g(X,Y)':b='b(X,Y)':a='X*4'" -c:v hap -format hap_alpha
make vc2.mkv yuv420p -c:v vc2 -b:v 2M -wavelet_depth 3 -pix_fmt yuv420p
make vc2-10.mov yuv422p10le -c:v vc2 -b:v 3M -pix_fmt yuv422p10le -wavelet_type 5_3
make flashsv.flv gbrp -c:v flashsv -g 2
make flashsv2.flv gbrp -c:v flashsv2 -g 2
# OGM (MPEG-4 Part 2) and Dirac in Ogg, from elementary frames.
ffmpeg $q -f lavfi -i "$src" -c:v mpeg4 -g 2 -bf 0 vg3-ogm-src.avi
rm -rf ogm.d && mkdir ogm.d
ffmpeg $q -i vg3-ogm-src.avi -map 0:v -c copy -f image2 ogm.d/%04d.bin
python3 -I oggwrite.py ogm vg3-ogm.ogm XVID 25 64 48 ogm.d
# The integer IDCT the MPEG-4 Part 2 decoder matches.
ffmpeg $q -idct int -flags +bitexact -i vg3-ogm-src.avi -map 0:v -f rawvideo -pix_fmt yuv420p vg3-ogm.yuv
rm -rf dirac.d && mkdir dirac.d
ffmpeg $q -f lavfi -i "$src" -c:v vc2 -b:v 2M -wavelet_depth 3 -pix_fmt yuv420p vg3-dirac-src.mkv
ffmpeg $q -i vg3-dirac-src.mkv -map 0:v -c copy -f image2 dirac.d/%04d.bin
python3 -I oggwrite.py dirac vg3-dirac.ogg dirac.d
ffmpeg $q -i vg3-dirac-src.mkv -map 0:v -f rawvideo -pix_fmt yuv420p vg3-dirac.yuv
# OGM sound: MP3 frames (two length bytes each) and 16-bit PCM (timed by size); FFmpeg's
# decode of each .ogm is the oracle (s16le), checked by sound_gaps.rs.
asrc="sine=frequency=440:sample_rate=44100:duration=0.3,pan=stereo|c0=c0|c1=0.5*c0"
ffmpeg $q -f lavfi -i "$asrc" -c:a libmp3lame -b:a 128k -write_xing 0 -id3v2_version 0 -f mp3 vg3-ogm-src.mp3
python3 -I oggwrite.py ogm-audio vg3-ogm-mp3.ogm 55 44100 2 0 vg3-ogm-src.mp3
ffmpeg $q -f lavfi -i "$asrc" -f s16le vg3-ogm-src.s16
python3 -I oggwrite.py ogm-audio vg3-ogm-pcm.ogm 1 44100 2 16 vg3-ogm-src.s16
for n in mp3 pcm; do ffmpeg $q -i vg3-ogm-$n.ogm -f s16le vg3-ogm-$n.pcm; done
rm -rf ogm.d dirac.d vg3-ogm-src.avi vg3-dirac-src.mkv vg3-ogm-src.mp3 vg3-ogm-src.s16
