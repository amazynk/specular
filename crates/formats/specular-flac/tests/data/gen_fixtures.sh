#!/bin/sh
# Synthetic FLAC streams from ffmpeg 8's encoder and libavcodec's decode of each as raw PCM
# (s16le for 16-bit, s32le left-aligned for 24/32-bit). Run from this directory: sh gen_fixtures.sh
set -e
tone='0.5*sin(2*PI*440*t)+0.01*(random(0)-0.5)'
tone2='0.4*sin(2*PI*659*t+1)*sin(2*PI*3*t)+0.005*(random(1)-0.5)'
tone3='0.3*sin(2*PI*1500*t)+0.02*(random(2)-0.5)'
# gen NAME RATE SECONDS EXPRS PCMFMT ENCODER-ARGS...
gen() {
    name=$1 rate=$2 dur=$3 exprs=$4 pcm=$5
    shift 5
    fmt=s32
    [ "$pcm" = s16le ] && fmt=s16
    ffmpeg -nostdin -loglevel error -y -f lavfi -i "aevalsrc=exprs=$exprs:s=$rate:d=$dur" \
        -c:a flac -sample_fmt $fmt "$@" "$name.flac"
    ffmpeg -nostdin -loglevel error -y -i "$name.flac" -f "$pcm" "$name.pcm"
}
st="$tone|$tone2"
gen c0 44100 0.2 "$st" s16le -compression_level 0
gen c5 44100 0.2 "$st" s16le -compression_level 5
gen c8 44100 0.2 "$st" s16le -compression_level 8 -lpc_type levinson
gen c12 44100 0.2 "$st" s16le -compression_level 12 -lpc_type cholesky -exact_rice_parameters 1
gen fixed 44100 0.15 "$st" s16le -compression_level 5 -lpc_type fixed
gen verb 44100 0.05 "$st" s16le -lpc_type none
gen ls 44100 0.15 "$st" s16le -compression_level 8 -ch_mode left_side
gen rs 44100 0.15 "$st" s16le -compression_level 8 -ch_mode right_side
gen ms 44100 0.15 "$st" s16le -compression_level 8 -ch_mode mid_side
gen small 44100 0.1 "$st" s16le -compression_level 5 -frame_size 576
gen m8k 8000 0.77 "$tone" s16le -compression_level 5
gen wasted 32000 0.2 "floor(($tone)*256)/256|floor(($tone2)*64)/64" s16le -compression_level 5
gen s24 96000 0.08 "$st" s32le -bits_per_raw_sample 24 -compression_level 8
gen s24ms 48000 0.1 "$st" s32le -bits_per_raw_sample 24 -compression_level 5 -ch_mode mid_side
gen s32 48000 0.08 "$st" s32le -strict experimental -compression_level 5
gen s51 48000 0.08 "$tone|$tone2|$tone3|$tone|$tone3|$tone2" s16le -compression_level 5
