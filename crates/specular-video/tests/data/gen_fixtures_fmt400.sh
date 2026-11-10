#!/bin/sh
# Remakes the fmt400 fixtures (more sound files, YUV4MPEG2, CD+G) and their oracles.
# sox and ffmpeg only write samples and decode them (the oracle); gen_fmt400.py writes what
# no installed tool writes. Run from this directory: sh gen_fixtures_fmt400.sh
set -eu
D=$(cd "$(dirname "$0")" && pwd)
T=$(mktemp -d "${TMPDIR:?set TMPDIR}/fmt400.XXXXXX")
trap 'rm -rf "$T"' EXIT
cd "$D"
F="ffmpeg -nostdin -v error -y"
# 0.2 s of 22.05 kHz stereo: two tones and a little noise.
sox -n -r 22050 -c 2 -b 16 "$T/src.wav" synth 0.2 sine 440 sine 660 synth 0.2 pinknoise mix gain -4
# The ffmpeg oracle of a fixture: 16-bit little-endian interleaved.
pcm() { $F -i "$1" -f s16le -acodec pcm_s16le "${1%.*}.pcm"; }

sox "$T/src.wav" -e signed -b 16 fmt400-au.au
sox "$T/src.wav" -c 1 -e u-law fmt400-ulaw.snd
$F -i "$T/src.wav" -c:a pcm_s16be fmt400-caf.caf
$F -i "$T/src.wav" -c:a pcm_f32le fmt400-f32.caf
$F -i "$T/src.wav" -c:a alac fmt400-alac.caf
$F -i "$T/src.wav" fmt400-w64.w64
$F -i "$T/src.wav" -rf64 always -f wav fmt400-rf64.rf64
sox "$T/src.wav" fmt400-voc.voc
sox "$T/src.wav" -b 8 fmt400-voc8.voc
sox "$T/src.wav" fmt400-8svx.8svx
sox "$T/src.wav" fmt400-avr.avr
sox "$T/src.wav" fmt400-sph.sph
sox "$T/src.wav" fmt400-ircam.sf
$F -i "$T/src.wav" -c:a tta fmt400-tta.tta
sox "$T/src.wav" -b 24 "$T/src24.wav"
$F -i "$T/src24.wav" -c:a tta fmt400-tta24.tta
for f in fmt400-au.au fmt400-ulaw.snd fmt400-caf.caf fmt400-f32.caf fmt400-alac.caf \
    fmt400-w64.w64 fmt400-rf64.rf64 fmt400-voc.voc fmt400-voc8.voc fmt400-8svx.8svx \
    fmt400-avr.avr fmt400-sph.sph fmt400-ircam.sf fmt400-tta.tta fmt400-tta24.tta; do
    pcm "$f"
done
python3 -I gen_fmt400.py "$D" "$T/src.wav"
pcm fmt400-shn.shn
$F -i fmt400-bw64.bw64 -f s16le -acodec pcm_s16le fmt400-bw64.pcm 2>/dev/null \
    || cp fmt400-rf64.pcm fmt400-bw64.pcm
# Modules: libopenmpt (through ffmpeg) renders the oracle, 48 kHz stereo.
for m in mod s3m xm it; do
    $F -i fmt400.$m -ar 48000 -ac 2 -f s16le -acodec pcm_s16le fmt400-$m.pcm
done
# YUV4MPEG2 and its frames as raw planes; CD+G and ffmpeg's RGB frames.
$F -f lavfi -i testsrc2=size=34x22:rate=25:duration=0.24 -pix_fmt yuv420p fmt400-420.y4m
$F -f lavfi -i testsrc2=size=34x22:rate=25:duration=0.24 -pix_fmt yuv422p10le -strict -1 fmt400-422p10.y4m
$F -f lavfi -i testsrc2=size=34x22:rate=25:duration=0.24 -pix_fmt yuv444p fmt400-444.y4m
for y in fmt400-420 fmt400-422p10 fmt400-444; do
    $F -i $y.y4m -f rawvideo $y.yuv
done
$F -i fmt400.cdg -fps_mode passthrough -f rawvideo -pix_fmt rgb24 fmt400-cdg.rgb
cp "$T/src.wav" fmt400.wav
python3 -I gen_fmt400.py --refs "$D"
# ffmpeg's Shorten decode must be what the coder meant.
cmp fmt400-shn.pcm fmt400-shn.src
rm fmt400-shn.src
