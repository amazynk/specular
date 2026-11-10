#!/bin/sh
# Motion JPEG, JPEG 2000, FLV and ASF, with ffmpeg's decodes
# as the oracle (.yuv: every frame as decoded, in the decoder's own pixel format; JPEG with
# `-idct int`, libjpeg's integer IDCT). Run from this directory: sh gen_fixtures_videocodecs_extra.sh
#   mjpeg.avi        Motion JPEG 4:2:2 (AVI MJPG), PCM sound
#   mjpeg420.mov     Motion JPEG 4:2:0 in QuickTime (`jpeg`)
#   mjpeg-nodht.mkv  Motion JPEG without Huffman tables (V_MJPEG; the AVI1 convention)
#   mjp2.mov         JPEG 2000 4:2:0 8-bit in QuickTime (`mjp2`)
#   j2k-422-10.mxf   JPEG 2000 4:2:2 10-bit in MXF
#   spark.flv        Sorenson Spark (FLV1) with MP3
#   h264-aac.flv     H.264 with B-frames and AAC in FLV
#   mpeg4.wmv        MPEG-4 Part 2 in ASF with 512-byte packets (frames split over packets)
#   msmpeg4.wmv      Windows Media Video 8 (refused, with its name)
set -e
q="-nostdin -loglevel error -y"
moving="-f lavfi -i mandelbrot=size=96x64:rate=25,trim=duration=0.4"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.4"
sine="-f lavfi -i sine=frequency=600:sample_rate=48000:duration=0.4"
ref() {
    ffmpeg $q -threads 1 -flags +bitexact -idct int -i "$1" -map 0:v:0 -fps_mode passthrough -f rawvideo "${1%.*}.yuv"
}
ffmpeg $q $moving $sine -ac 2 -c:v mjpeg -pix_fmt yuvj422p -q:v 4 -c:a pcm_s16le mjpeg.avi
ref mjpeg.avi
ffmpeg $q $pic -c:v mjpeg -pix_fmt yuvj420p -q:v 3 mjpeg420.mov
ref mjpeg420.mov
# Default (Annex K) tables, then the DHT segments taken out of every frame.
ffmpeg $q $pic -c:v mjpeg -huffman default -pix_fmt yuvj420p -q:v 3 -f mjpeg - \
    | python3 -I -c '
import sys
d = sys.stdin.buffer.read()
out = bytearray()
i = 0
while i < len(d):
    if d[i] == 0xFF and d[i + 1] == 0xC4:
        n = (d[i + 2] << 8) | d[i + 3]
        i += 2 + n
        continue
    if d[i] == 0xFF and d[i + 1] == 0xDA:
        j = d.index(b"\xff\xd9", i) + 2
        out += d[i:j]
        i = j
        continue
    out.append(d[i])
    i += 1
sys.stdout.buffer.write(bytes(out))
' > nodht.mjpeg
ffmpeg $q -f mjpeg -framerate 25 -i nodht.mjpeg -c copy mjpeg-nodht.mkv
rm nodht.mjpeg
ref mjpeg-nodht.mkv
ffmpeg $q $moving -c:v jpeg2000 -pix_fmt yuv420p -q:v 6 mjp2.mov
ref mjp2.mov
ffmpeg $q $pic -c:v jpeg2000 -pix_fmt yuv422p10le -q:v 8 j2k-422-10.mxf
ref j2k-422-10.mxf
ffmpeg $q $moving -f lavfi -i sine=frequency=600:sample_rate=44100:duration=0.4 -c:v flv -q:v 4 -g 5 -c:a libmp3lame -b:a 64k spark.flv
ref spark.flv
ffmpeg $q $moving -f lavfi -i sine=frequency=600:sample_rate=44100:duration=0.4 -c:v libx264 -pix_fmt yuv420p -bf 2 -g 10 -c:a aac -b:a 64k h264-aac.flv
ref h264-aac.flv
ffmpeg $q $moving $sine -c:v mpeg4 -bf 0 -g 5 -q:v 2 -c:a pcm_s16le -packet_size 512 mpeg4.wmv
ref mpeg4.wmv
ffmpeg $q $pic -frames:v 3 -c:v wmv2 msmpeg4.wmv
# VP9 4:4:4 (vp9-444.webm comes from gen_fixtures_mkv_ts.sh).
ffmpeg $q -threads 1 -i vp9-444.webm -map 0:v:0 -fps_mode passthrough -f rawvideo vp9-444.yuv
# j2k.mxf (from gen_fixtures_provideo.sh): ffmpeg's decode.
ffmpeg $q -i j2k.mxf -f rawvideo j2k.yuv
# The Motion JPEG above as a raw stream (tests/raw_streams.rs; oracle mjpeg.yuv).
"${FF[@]}" -i mjpeg.avi -map 0:v -c:v copy -f mjpeg es.mjpeg
