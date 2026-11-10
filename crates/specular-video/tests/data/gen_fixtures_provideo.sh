#!/bin/sh
# Professional video (ffmpeg 8.0): ProRes in QuickTime, Matroska and MXF, MXF with MPEG-2,
# D-10, H.264 and PCM, and the codings MXF refuses. FFmpeg's encoders make the files and
# its decodes are the oracle: .yuv (every frame as decoded, both fields woven) and .pcm
# (s16le). Run from this directory: sh gen_fixtures_provideo.sh
#   prores-hq.mov      ProRes 422 HQ
#   prores-proxy.mov   ProRes 422 Proxy, a size that is not a multiple of 16
#   prores-4444a.mov   ProRes 4444 with a 16-bit alpha channel (yuva444p12le)
#   prores-xq.mkv      ProRes 4444 XQ in Matroska
#   prores-il.mov      ProRes 422 interlaced, bottom field first
#   prores-pcm.mxf     OP1a, frame-wrapped ProRes 422 LT and 24-bit PCM
#   prores-atom.mxf    OP-Atom, clip-wrapped ProRes 422 (index table in the footer)
#   mpeg2-pcm.mxf      OP1a, long-GOP MPEG-2 4:2:2 interlaced (B-frames) and 16-bit PCM
#   d10.mxf            D-10 (IMX 30): intra MPEG-2 4:2:2, 720×608, AES3 sound (d10.fnv:
#                      frame hashes)
#   h264-pcm.mxf       OP1a, long-GOP H.264 (B-frames) and PCM
#   dnxhr.mxf, j2k.mxf codings MXF refuses by name
set -e
q="-nostdin -loglevel error -y"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.16"
snd="-f lavfi -i sine=frequency=500:sample_rate=48000:duration=0.16"
ref() {
    ffmpeg $q -i "$1" -map 0:v:0 -fps_mode passthrough -f rawvideo -pix_fmt "$2" "${1%.*}.yuv"
}
pcm() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s16le -ac "$2" "${1%.*}.pcm"
}
ffmpeg $q $pic -c:v prores_ks -profile:v 3 -pix_fmt yuv422p10le prores-hq.mov
ref prores-hq.mov yuv422p10le
ffmpeg $q -f lavfi -i testsrc2=size=90x50:rate=25:duration=0.12 -c:v prores_ks -profile:v 0 -pix_fmt yuv422p10le prores-proxy.mov
ref prores-proxy.mov yuv422p10le
ffmpeg $q -f lavfi -i "testsrc2=size=96x64:rate=25:duration=0.12,format=yuva444p10le,geq=lum='lum(X,Y)':cb='cb(X,Y)':cr='cr(X,Y)':a='X*10'" -c:v prores_ks -profile:v 4 -alpha_bits 16 -pix_fmt yuva444p10le prores-4444a.mov
ref prores-4444a.mov yuva444p12le
ffmpeg $q -f lavfi -i testsrc2=size=96x64:rate=25:duration=0.12 -c:v prores_ks -profile:v 5 -pix_fmt yuv444p12le prores-xq.mkv
ref prores-xq.mkv yuv444p12le
ffmpeg $q -f lavfi -i testsrc2=size=96x64:rate=25:duration=0.16 -vf "setfield=bff,interlace=scan=bff" -c:v prores_ks -profile:v 2 -flags +ildct -pix_fmt yuv422p10le prores-il.mov
ref prores-il.mov yuv422p10le
ffmpeg $q $pic $snd -c:v prores_ks -profile:v 1 -pix_fmt yuv422p10le -c:a pcm_s24le prores-pcm.mxf
ref prores-pcm.mxf yuv422p10le
pcm prores-pcm.mxf 1
ffmpeg $q $pic -c:v prores_ks -profile:v 2 -pix_fmt yuv422p10le -f mxf_opatom prores-atom.mxf
ref prores-atom.mxf yuv422p10le
ffmpeg $q -f lavfi -i testsrc2=size=96x64:rate=25:duration=0.4 -f lavfi -i sine=frequency=500:sample_rate=48000:duration=0.4 -c:v mpeg2video -pix_fmt yuv422p -flags +ildct+ilme -top 1 -g 6 -bf 2 -q:v 4 -c:a pcm_s16le mpeg2-pcm.mxf
ffmpeg $q -idct int -i mpeg2-pcm.mxf -map 0:v:0 -fps_mode passthrough -f rawvideo mpeg2-pcm.yuv
pcm mpeg2-pcm.mxf 1
ffmpeg $q -f lavfi -i testsrc2=size=720x608:rate=25:duration=0.08 -f lavfi -i sine=frequency=500:sample_rate=48000:duration=0.08 -c:v mpeg2video -pix_fmt yuv422p -b:v 30M -minrate 30M -maxrate 30M -bufsize 1200000 -rc_init_occupancy 1200000 -g 1 -flags +ildct+ilme -top 1 -dc 10 -intra_vlc 1 -non_linear_quant 1 -qmin 1 -qmax 12 -ps 1 -c:a pcm_s16le -f mxf_d10 d10.mxf
ffmpeg $q -idct int -i d10.mxf -map 0:v:0 -fps_mode passthrough -f rawvideo d10.yuv
# D-10 frames are big: keep each frame's 64-bit FNV-1a hash (d10.fnv), not the frames.
python3 -c "
import sys
d = open('d10.yuv', 'rb').read(); n = 720 * 608 * 2
for i in range(0, len(d), n):
    h = 0xcbf29ce484222325
    for b in d[i:i + n]:
        h = ((h ^ b) * 0x100000001b3) & 0xffffffffffffffff
    print('%016x' % h)
" > d10.fnv
rm d10.yuv
pcm d10.mxf 1
ffmpeg $q -f lavfi -i testsrc2=size=96x64:rate=25:duration=0.4 -f lavfi -i sine=frequency=500:sample_rate=48000:duration=0.4 -c:v libx264 -pix_fmt yuv420p -g 6 -bf 2 -c:a pcm_s24le h264-pcm.mxf
ref h264-pcm.mxf yuv420p
ffmpeg $q -f lavfi -i testsrc2=size=256x144:rate=25:duration=0.04 -c:v dnxhd -profile:v dnxhr_lb -pix_fmt yuv422p dnxhr.mxf
ffmpeg $q -f lavfi -i testsrc2=size=96x64:rate=25:duration=0.04 -c:v jpeg2000 j2k.mxf
