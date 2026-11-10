#!/bin/sh
# Synthetic MPEG-4 Part 2 and H.263 streams and their ffmpeg frame MD5s (ffmpeg 8.0 with
# libxvid; decoded on one thread with the integer IDCT the decoder matches, bit-exact flags:
# libavcodec's frame-threaded decode of field direct mode differs from its own). Run from this
# directory: sh gen_fixtures.sh
set -e
q="-nostdin -loglevel error -y"
src="-f lavfi -i testsrc2=size=176x144:rate=25:duration=0.48"
moving="-f lavfi -i mandelbrot=size=176x144:rate=25 -frames:v 12"
ref() {
    ffmpeg $q -threads 1 -flags +bitexact -idct int -i "$1" -fps_mode passthrough -f framemd5 - | grep -v '^#' > "${1%.*}.framemd5"
}
enc() {
    name=$1; shift
    ffmpeg $q "$@" "$name"
    ref "$name"
}
# Simple Profile: I and P, H.263 quantisation, AC prediction.
enc sp.m4v $src -c:v mpeg4 -bf 0 -g 6 -q:v 4 -f m4v
# Four vectors per macroblock, MPEG quantisation.
enc mv4.m4v $moving -c:v mpeg4 -flags +mv4 -mpeg_quant 1 -bf 0 -g 12 -q:v 3 -f m4v
# B-VOPs.
enc bvop.m4v $moving -c:v mpeg4 -bf 2 -g 12 -q:v 4 -f m4v
# B-VOPs over a still background: macroblocks whose co-located P macroblock was skipped carry
# no bits, and FFmpeg starts video packets right after them (they were
# jumped over and left black).
enc bvop-skips.m4v -f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 8 -c:v mpeg4 -bf 2 -g 6 -q:v 4 -f m4v
# Quarter-sample motion with B-VOPs.
enc qpel.m4v $moving -c:v mpeg4 -flags +qpel+mv4 -bf 2 -g 12 -q:v 3 -f m4v
# Interlaced: field DCT and field prediction.
enc interlaced.m4v $moving -vf "tinterlace=interleave_top,setfield=tff" -c:v mpeg4 -flags +ildct+ilme -bf 1 -g 12 -q:v 3 -f m4v
# Video packets (resync markers).
enc resync.m4v $moving -c:v mpeg4 -ps 400 -bf 0 -g 12 -q:v 2 -f m4v
# Xvid: global motion compensation and quarter samples.
# (Panning and zooming pictures: Xvid codes S-VOPs for them.)
enc xvid-gmc.m4v -f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 12 -vf scroll=horizontal=0.004:vertical=0.003 -c:v libxvid -gmc 1 -bf 0 -g 12 -q:v 3 -f m4v
enc xvid-gmc-zoom.m4v -f lavfi -i testsrc2=size=176x144:rate=25 -frames:v 12 -vf zoompan=z=1+0.01*on:d=1:s=176x144 -c:v libxvid -gmc 1 -bf 0 -g 12 -q:v 3 -f m4v
enc xvid-qpel.m4v $moving -c:v libxvid -flags +qpel -bf 2 -g 12 -q:v 3 -f m4v
# H.263 baseline.
enc baseline.h263 $src -c:v h263 -g 6 -q:v 5 -f h263
# Data partitioning: video packets, B-VOPs (never partitioned), adaptive quantisers (dquant);
# then four vectors and MPEG quantisation.
enc partitioned.m4v $moving -c:v mpeg4 -data_partitioning 1 -ps 600 -bf 1 -g 6 -q:v 4 -lumi_mask 0.3 -f m4v
enc partitioned-mv4.m4v $moving -c:v mpeg4 -data_partitioning 1 -ps 300 -flags +mv4 -mpeg_quant 1 -bf 0 -g 12 -q:v 2 -f m4v
