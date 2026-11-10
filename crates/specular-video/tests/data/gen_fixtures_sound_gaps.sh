#!/bin/sh
# Sound gaps (format gaps item 12): ffmpeg 8.0 makes each fixture, and its decode is the
# oracle (.pcm s16le, or .s32 s32le, at the stream's rate and channels).
# Run from this directory: sh gen_fixtures_sound_gaps.sh
#   lpcm24.vob      MPEG-2 with 24-bit DVD LPCM 5.1 48 kHz (.s32)
#   lpcm24-96.vob   with 24-bit DVD LPCM stereo 96 kHz (.s32)
#   lpcm20.vob      sound only, 20-bit DVD LPCM stereo 48 kHz (lpcm_ps.py: ffmpeg has no
#                   20-bit encoder), groups split across packets (.s32)
#   lpcm24-odd.vob  sound only, 24-bit 5.1 the same way (.s32)
set -e
q="-nostdin -loglevel error -y"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.2"
tone2() { echo "-f lavfi -i aevalsrc=0.4*sin(2*PI*$1*t)|0.3*sin(2*PI*$2*t)+0.01*random(0):s=$3:d=0.2"; }
tone6="-f lavfi -i aevalsrc=0.4*sin(2*PI*300*t)|0.3*sin(2*PI*500*t)|0.2*random(1)|0.1*sin(2*PI*60*t)|0.3*sin(2*PI*900*t)|0.02*random(2):s=48000:d=0.2"
s32() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s32le "${1%.*}.s32"
}
lpcm="-c:v mpeg2video -q:v 6 -c:a pcm_dvd -f vob"
ffmpeg $q $pic $tone6 $lpcm -sample_fmt s32 lpcm24.vob && s32 lpcm24.vob
ffmpeg $q $pic $(tone2 1000 3000 96000) $lpcm -sample_fmt s32 lpcm24-96.vob && s32 lpcm24-96.vob
python3 -I lpcm_ps.py 20 2 48000 0.3 lpcm20.vob && s32 lpcm20.vob
python3 -I lpcm_ps.py 24 6 48000 0.1 lpcm24-odd.vob && s32 lpcm24-odd.vob
# Free-format MPEG audio: constant-rate streams with the bit-rate index set to 0
# (free_format.py), so the frame length must be found from the next header.
#   free-mp2.mp2    Layer II stereo 48 kHz (192 kbit/s, no padding)
#   free-mp3.mp3    Layer III stereo 44.1 kHz (128 kbit/s, padded frames)
#   free-mp3lsf.mp3 Layer III mono 22.05 kHz (MPEG-2, 48 kbit/s)
#   free-mkv.mkv    Layer III stereo 48 kHz in Matroska (one frame a block)
# ffmpeg won't open free-format files, so the oracle is its decode of the constant-rate
# stream before the rewrite (the coded sound is the same).
pcm() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s16le "$2.pcm"
}
ffmpeg $q $(tone2 440 700 48000) -c:a mp2 -b:a 192k cbr.mp2 && python3 -I free_format.py cbr.mp2 free-mp2.mp2 && pcm cbr.mp2 free-mp2
ffmpeg $q $(tone2 500 900 44100) -c:a libmp3lame -b:a 128k -write_xing 0 cbr.mp3 && python3 -I free_format.py cbr.mp3 free-mp3.mp3 && pcm cbr.mp3 free-mp3
ffmpeg $q $(tone2 600 800 22050) -ac 1 -c:a libmp3lame -b:a 48k -write_xing 0 cbr.mp3 && python3 -I free_format.py cbr.mp3 free-mp3lsf.mp3 && pcm cbr.mp3 free-mp3lsf
ffmpeg $q $(tone2 700 300 48000) -c:a libmp3lame -b:a 160k cbr.mkv && python3 -I free_format.py cbr.mkv free-mkv.mkv && pcm cbr.mkv free-mkv
rm -f cbr.mp2 cbr.mp3 cbr.mkv
# Opus mapping families 2, 3 and 255 (libopus); the oracle is libopus's decode (ffmpeg
# -c:a libopus) as the player renders it (opus_amb.py render, .s32 stereo).
#   opus-foa.ogg    family 2, first-order ambisonics (4 channels)
#   opus-foa6.ogg   family 2, first order plus non-diegetic stereo (6 channels)
#   opus-fam3.ogg   opus-foa.ogg rewritten as family 3 with a demixing matrix
#   opus-255.ogg    family 255, 3 channels without a layout
amb4="-f lavfi -i aevalsrc=0.3*sin(2*PI*300*t)|0.2*sin(2*PI*500*t)|0.1*sin(2*PI*700*t)|0.15*sin(2*PI*900*t):s=48000:d=0.3"
amb6="-f lavfi -i aevalsrc=0.3*sin(2*PI*300*t)|0.2*sin(2*PI*500*t)|0.1*sin(2*PI*700*t)|0.15*sin(2*PI*900*t)|0.1*sin(2*PI*1200*t)|0.1*sin(2*PI*1500*t):s=48000:d=0.3"
amb3="-f lavfi -i aevalsrc=0.3*sin(2*PI*300*t)|0.2*sin(2*PI*500*t)|0.1*sin(2*PI*700*t):s=48000:d=0.3"
lib="-c:a libopus -b:a 192k"
f32() {
    ffmpeg $q -c:a libopus -i "$1" -f f32le opus.f32
}
ffmpeg $q $amb4 $lib -mapping_family 2 opus-foa.ogg && f32 opus-foa.ogg && python3 -I opus_amb.py render opus.f32 4 opus-foa.s32
ffmpeg $q $amb6 $lib -mapping_family 2 opus-foa6.ogg && f32 opus-foa6.ogg && python3 -I opus_amb.py render opus.f32 6 opus-foa6.s32
python3 -I opus_amb.py family3 opus-foa.ogg opus-fam3.ogg && f32 opus-foa.ogg && python3 -I opus_amb.py render opus.f32 4 opus-fam3.s32 family3
ffmpeg $q $amb3 $lib -mapping_family 255 opus-255.ogg && f32 opus-255.ogg && python3 -I opus_amb.py render opus.f32 2553 opus-255.s32
rm -f opus.f32
# Flash ADPCM in FLV (4-bit codes, ffmpeg's adpcm_swf), oracle .pcm.
#   swf-mono.flv    mono 22.05 kHz, with Sorenson Spark
#   swf-stereo.flv  stereo 44.1 kHz, sound only
pcm1() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s16le "${1%.*}.pcm"
}
ffmpeg $q $pic $(tone2 440 700 22050) -ac 1 -c:v flv -c:a adpcm_swf swf-mono.flv && pcm1 swf-mono.flv
ffmpeg $q $(tone2 500 900 44100) -c:a adpcm_swf -t 0.2 swf-stereo.flv && pcm1 swf-stereo.flv
# Nellymoser in FLV (ffmpeg's encoder), oracle .pcm. Lines without bits are noise of random
# sign, so the comparison is a signal-to-noise ratio.
#   nelly-8k.flv    8 kHz (FLV format 5)
#   nelly-16k.flv   16 kHz (format 4), with Sorenson Spark
#   nelly-44k.flv   44.1 kHz (format 6)
ffmpeg $q $(tone2 300 1100 8000) -ac 1 -c:a nellymoser -t 0.3 nelly-8k.flv && pcm1 nelly-8k.flv
ffmpeg $q $pic $(tone2 400 1500 16000) -ac 1 -c:v flv -c:a nellymoser nelly-16k.flv && pcm1 nelly-16k.flv
ffmpeg $q $(tone2 500 2500 44100) -ac 1 -c:a nellymoser nelly-44k.flv && pcm1 nelly-44k.flv
# Speex (libspeex), oracle: libspeex's decode through ffmpeg (-c:a libspeex), .pcm.
#   spx-nb4.ogg     narrowband quality 4 (8 kbit/s)
#   spx-nb8.ogg     narrowband quality 8 (15 kbit/s), 2 frames a packet
#   spx-nb10.ogg    narrowband quality 10 (24.6 kbit/s, double codebook)
#   spx-wb.ogg      wideband quality 8
#   spx-uwb.ogg     ultra-wideband quality 8
#   spx-stereo.ogg  narrowband intensity stereo
#   spx.flv         wideband mono in FLV
voice="aevalsrc=0.3*sin(2*PI*(180+40*sin(2*PI*3*t))*t)*(0.6+0.4*sin(2*PI*2*t))+0.1*sin(2*PI*1300*t)+0.02*random(0)"
spx() {
    ffmpeg $q -c:a libspeex -i "$1" -map 0:a:0 -f s16le "${1%.*}.pcm"
}
ffmpeg $q -f lavfi -i "$voice:s=8000:d=0.6" -c:a libspeex -cbr_quality 4 spx-nb4.ogg && spx spx-nb4.ogg
ffmpeg $q -f lavfi -i "$voice:s=8000:d=0.6" -c:a libspeex -cbr_quality 8 -frames_per_packet 2 spx-nb8.ogg && spx spx-nb8.ogg
ffmpeg $q -f lavfi -i "$voice:s=8000:d=0.6" -c:a libspeex -cbr_quality 10 spx-nb10.ogg && spx spx-nb10.ogg
ffmpeg $q -f lavfi -i "$voice:s=16000:d=0.6" -c:a libspeex -cbr_quality 8 spx-wb.ogg && spx spx-wb.ogg
ffmpeg $q -f lavfi -i "$voice:s=32000:d=0.6" -c:a libspeex -cbr_quality 8 spx-uwb.ogg && spx spx-uwb.ogg
ffmpeg $q -f lavfi -i "$voice:s=8000:d=0.6" -af "pan=stereo|c0=0.9*c0|c1=0.4*c0" -c:a libspeex -cbr_quality 8 spx-stereo.ogg && spx spx-stereo.ogg
ffmpeg $q -f lavfi -i "$voice:s=16000:d=0.6" -c:a libspeex -cbr_quality 8 spx.flv && spx spx.flv
