#!/bin/sh
# Sound in video: G.711, IMA and Microsoft ADPCM, ALAC and MP3 in their containers
# (ffmpeg 8.0 with libmp3lame), with ffmpeg's decodes as the oracle: .pcm (s16le at
# the stream's rate and channels).
# Run from this directory: sh gen_fixtures_sound_adpcm_alac_mp3.sh
#   ulaw.avi        MPEG-4 Part 2 with G.711 µ-law mono 8 kHz (WAVE tag 7)
#   alaw.mov        with G.711 A-law stereo 8 kHz (`alaw`)
#   flv-alaw.flv    Sorenson Spark with G.711 A-law (FLV sound format 7)
#   ima.avi         with IMA ADPCM stereo 22.05 kHz (WAVE tag 0x11)
#   ima4.mov        with QuickTime IMA ADPCM stereo (`ima4`)
#   msadpcm.avi     with Microsoft ADPCM stereo (WAVE tag 2)
#   msadpcm-mono.mkv  with Microsoft ADPCM mono (A_MS/ACM)
#   alac.mp4        with ALAC 16-bit stereo 44.1 kHz
#   alac-mono.mkv   VP8 with ALAC mono 48 kHz (A_ALAC)
#   alac24.mov      with ALAC 24-bit stereo 96 kHz (.s32: s32le oracle)
#   alac51.mov      with ALAC 24-bit 5.1 48 kHz (.s32)
#   mp3-avi.avi … mp3-mpg.mpg  MP3 (LAME) in each container
set -e
q="-nostdin -loglevel error -y"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.4"
snd() { echo "-f lavfi -i sine=frequency=$1:sample_rate=$2:duration=0.4"; }
pcm() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s16le "${1%.*}.pcm"
}
# From time 0 on (Matroska keeps the encoder's priming before it).
pcm0() {
    ffmpeg $q -copyts -i "$1" -map 0:a:0 -af atrim=start=0 -f s16le "${1%.*}.pcm"
}
mp4v="-c:v mpeg4 -q:v 4 -g 6"
ffmpeg $q $pic $(snd 500 8000) -ac 1 $mp4v -c:a pcm_mulaw ulaw.avi && pcm ulaw.avi
ffmpeg $q $pic $(snd 600 8000) -ac 2 $mp4v -c:a pcm_alaw alaw.mov && pcm alaw.mov
ffmpeg $q $pic $(snd 650 8000) -ac 1 -c:v flv -g 6 -c:a pcm_alaw flv-alaw.flv && pcm flv-alaw.flv
ffmpeg $q $pic $(snd 700 22050) -ac 2 $mp4v -c:a adpcm_ima_wav ima.avi && pcm ima.avi
ffmpeg $q $pic $(snd 800 22050) -ac 2 $mp4v -c:a adpcm_ima_qt ima4.mov && pcm ima4.mov
ffmpeg $q $pic $(snd 900 22050) -ac 2 $mp4v -c:a adpcm_ms msadpcm.avi && pcm msadpcm.avi
ffmpeg $q $pic $(snd 1000 22050) -ac 1 $mp4v -c:a adpcm_ms msadpcm-mono.mkv && pcm msadpcm-mono.mkv
# ALAC: a quiet start (zero runs), then a tone over noise.
mix() { echo "-f lavfi -i aevalsrc=0.4*sin(2*PI*$1*t)*gt(t\\,0.15)+0.02*random(0)*gt(t\\,0.1):s=$2:d=0.4"; }
mix6="-f lavfi -i aevalsrc=0.4*sin(2*PI*300*t)|0.3*sin(2*PI*500*t)|0.2*random(1)|0.1*sin(2*PI*60*t)|0.3*sin(2*PI*900*t)*gt(t\\,0.08)|0.02*random(2):s=48000:d=0.4"
s32() {
    ffmpeg $q -i "$1" -map 0:a:0 -f s32le "${1%.*}.s32"
}
ffmpeg $q $pic $(mix 440 44100) -ac 2 $mp4v -c:a alac -sample_fmt s16p alac.mp4 && pcm alac.mp4
ffmpeg $q $pic $(mix 550 48000) -ac 1 -c:v libvpx -b:v 200k -g 6 -c:a alac -sample_fmt s16p alac-mono.mkv && pcm alac-mono.mkv
ffmpeg $q $pic $(mix 660 96000) -ac 2 $mp4v -c:a alac -sample_fmt s32p -t 0.16 alac24.mov && s32 alac24.mov
ffmpeg $q $pic $mix6 -map 0:v -map 1:a $mp4v -c:a alac -sample_fmt s32p -t 0.16 alac51.mov && s32 alac51.mov
# MP3 (LAME) in every container that carries it; .pcm as ffmpeg plays it (MP4 edit lists
# applied).
mp3="-c:a libmp3lame -b:a 96k"
ffmpeg $q $pic $(mix 440 44100) -ac 2 $mp4v $mp3 mp3-avi.avi && pcm mp3-avi.avi
ffmpeg $q $pic $(mix 500 44100) -ac 2 $mp4v $mp3 mp3-mp4.mp4 && pcm mp3-mp4.mp4
ffmpeg $q $pic $(mix 550 48000) -ac 2 -c:v libvpx -b:v 200k -g 6 $mp3 mp3-mkv.mkv && pcm0 mp3-mkv.mkv
ffmpeg $q $pic $(mix 600 22050) -ac 1 -c:v flv -g 6 $mp3 mp3-flv.flv && pcm mp3-flv.flv
ffmpeg $q $pic $(mix 650 48000) -ac 2 -c:v mpeg2video -q:v 4 -g 6 $mp3 -f mpegts mp3-ts.ts && pcm mp3-ts.ts
ffmpeg $q $pic $(mix 700 48000) -ac 2 -c:v mpeg1video -q:v 4 -g 6 $mp3 -f mpeg mp3-mpg.mpg && pcm mp3-mpg.mpg
# MXF: four mono 24-bit tracks in OP1a (channels 1 and 2 play as left and right), and an
# OP-Atom clip: the picture in one file, channels 1 and 2 in files of their own beside it
# (bit-exact muxing gives the three the same material package, as a camera's do).
ch() { echo "-f lavfi -i sine=frequency=$1:sample_rate=48000:duration=0.4"; }
prores="-c:v prores_ks -profile:v 1 -pix_fmt yuv422p10le"
ffmpeg $q $pic $(ch 300) $(ch 500) $(ch 700) $(ch 900) -map 0:v -map 1:a -map 2:a -map 3:a -map 4:a $prores -c:a pcm_s24le mxf-4mono.mxf
ffmpeg $q -i mxf-4mono.mxf -filter_complex "[0:a:0][0:a:1]amerge=inputs=2" -f s32le mxf-4mono.s32
mkdir -p opatom
ffmpeg $q $pic $prores -fflags +bitexact -f mxf_opatom opatom/clip_V1.mxf
ffmpeg $q $(ch 440) -c:a pcm_s24le -fflags +bitexact -f mxf_opatom opatom/clip_A1.mxf
ffmpeg $q $(ch 880) -c:a pcm_s24le -fflags +bitexact -f mxf_opatom opatom/clip_A2.mxf
ffmpeg $q -i opatom/clip_A1.mxf -i opatom/clip_A2.mxf -filter_complex "[0:a][1:a]amerge=inputs=2" -f s32le opatom/clip_V1.s32
