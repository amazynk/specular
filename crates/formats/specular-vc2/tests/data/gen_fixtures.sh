#!/bin/sh
# VC-2 fixtures. NAME.drc: a raw VC-2 stream, written by FFmpeg 8.0's vc2 encoder (high-quality
# pictures, four wavelets) or by vc2_writer.py (own writer: low-delay and high-quality pictures,
# every wavelet, field coding, custom matrices, prefix bytes). NAME.yuv: FFmpeg's decode of
# NAME.drc (a tool only, nothing of it is linked), planar, 8-bit or 16-bit little-endian.
# Where FFmpeg departs from the specification (Daubechies (9,7) lifting, and deep transforms
# whose smallest bands are one or two samples wide: ±1-3 in some samples), NAME.yuv is
# instead vc2_model.py's output, a direct transcription of the specification's decoding
# steps, which agrees with FFmpeg on every other fixture; the FFmpeg difference is printed.
# fixtures.txt: name width height chroma(444/422/420) bits.
# Run from this directory: sh gen_fixtures.sh
set -e
q="-nostdin -loglevel error -y"
: > fixtures.txt
oracle() {
    name=$1 w=$2 h=$3 chroma=$4 bits=$5
    ffmpeg $q -f dirac -i "$name.drc" -f rawvideo "$name.yuv"
    echo "$name $w $h $chroma $bits" >> fixtures.txt
}
ff() {
    name=$1 size=$2 pix=$3 chroma=$4 bits=$5
    shift 5
    ffmpeg $q -f lavfi -i "testsrc2=size=$size:rate=25,noise=alls=10:allf=t" -frames 2 \
        -c:v vc2 -pix_fmt "$pix" "$@" -f dirac "$name.drc"
    oracle "$name" "${size%x*}" "${size#*x}" "$chroma" "$bits"
}
own() {
    name=$1 w=$2 h=$3 chroma=$4 bits=$5
    shift 5
    python3 -I vc2_writer.py "$name.drc" "w=$w" "h=$h" "$@"
    oracle "$name" "$w" "$h" "$chroma" "$bits"
}
modelled() {
    name=$1 w=$2 h=$3 chroma=$4 bits=$5
    shift 5
    python3 -I vc2_writer.py "$name.drc" "w=$w" "h=$h" "$@"
    python3 -I vc2_model.py "$name.yuv" "w=$w" "h=$h" "$@"
    ffmpeg $q -f dirac -i "$name.drc" -f rawvideo "$name.ffmpeg.yuv"
    python3 -I -c 'import sys
a, b = (open(f, "rb").read() for f in sys.argv[1:3])
step = 2 if int(sys.argv[3]) > 8 else 1
n = sum(a[i:i + step] != b[i:i + step] for i in range(0, len(a), step))
print(sys.argv[4], "FFmpeg differs from the model in", n, "of", len(a) // step, "samples")' \
        "$name.yuv" "$name.ffmpeg.yuv" "$bits" "$name"
    rm "$name.ffmpeg.yuv"
    echo "$name $w $h $chroma $bits" >> fixtures.txt
}
# Field coding: FFmpeg refuses it, so the same field pictures written as half-height frames
# are decoded and woven (weave.py).
fields() {
    name=$1 w=$2 h=$3 chroma=$4 bits=$5 bps=$6
    shift 6
    python3 -I vc2_writer.py "$name.drc" "w=$w" "h=$h" fields=1 interlaced=1 "$@"
    python3 -I vc2_writer.py "$name.half.drc" "w=$w" "h=$((h / 2))" frames=4 "$@"
    ffmpeg $q -f dirac -i "$name.half.drc" -f rawvideo "$name.half.yuv"
    python3 -I weave.py "$name.half.yuv" "$name.yuv" "$w" "$h" "$chroma" "$bps"
    rm "$name.half.drc" "$name.half.yuv"
    echo "$name $w $h $chroma $bits" >> fixtures.txt
}
ff ff-dd97-420 64x48 yuv420p 420 8 -wavelet_type 9_7 -wavelet_depth 3 -b:v 3M
ff ff-legall-422-10 64x48 yuv422p10le 422 10 -wavelet_type 5_3 -qm color -b:v 4M
ff ff-haar-444-12 64x48 yuv444p12le 444 12 -wavelet_type haar -qm flat -wavelet_depth 2 -b:v 6M
ff ff-haarns-420 96x64 yuv420p 420 8 -wavelet_type haar_noshift -wavelet_depth 2 -b:v 1M -slice_width 64 -slice_height 32
own ld-dd97-420 64 48 420 8 syntax=ld wavelet=0 depth=3 chroma=2 range=1 bytes=160/1
own ld-legall-422-10 64 48 422 10 syntax=ld wavelet=1 depth=2 chroma=1 range=3 qm=custom bytes=445/2 seed=2
own ld-dd137-444 64 32 444 8 syntax=ld wavelet=2 depth=4 chroma=0 range=2 sx=2 sy=2 bytes=555/1 seed=3
fields ld-haar-fields 64 48 420 8 1 syntax=ld wavelet=3 depth=2 chroma=2 bytes=86/1 seed=4
own ld-fidelity 64 48 420 8 syntax=ld wavelet=5 depth=4 chroma=2 bytes=970/3 seed=5 sx=2 sy=3
modelled ld-daub97-12 64 48 420 12 syntax=ld wavelet=6 depth=3 chroma=2 range=4 bytes=154/1 seed=6
own ld-tight 64 48 422 8 syntax=ld wavelet=1 depth=3 chroma=1 bytes=199/1 seed=7
own hq-fidelity-prefix 64 48 420 8 syntax=hq wavelet=5 depth=3 chroma=2 prefix=2 scaler=2 seed=8
modelled hq-daub97-444-12 64 32 444 12 syntax=hq wavelet=6 depth=3 chroma=0 range=4 seed=9
modelled hq-dd137-deep 64 64 422 10 syntax=hq wavelet=2 depth=5 chroma=1 range=3 qm=custom sx=2 sy=2 scaler=4 seed=10
modelled hq-haar-shift 64 48 420 8 syntax=hq wavelet=4 depth=4 chroma=2 seed=11
fields hq-fields-422 64 48 422 10 2 syntax=hq wavelet=1 depth=2 chroma=1 range=3 seed=12
