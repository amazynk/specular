#!/usr/bin/env bash
# Regenerates the Matroska, WebM and MPEG-TS fixtures of tests/containers.rs with ffmpeg (8.0;
# libx264, libx265, libaom, libvpx, libopus, libvorbis). Every clip is 64x48, under a second:
# about 0.6 MB together. The .yuv / .pcm files are ffmpeg's own decodes, the oracle the tests
# compare against.
#
#   h264-opus.mkv    H.264 High with B-frames, Opus stereo (CELT); .yuv, .pcm (libopus decode)
#   vp9-opus.webm    VP9 profile 0 with hidden alt-ref frames, Opus mono (SILK); .yuv, .pcm
#   vp9-10bit.webm   VP9 profile 2 (10-bit); .yuv (yuv420p10le)
#   vp9-444.webm     VP9 profile 1 in 4:4:4 (vp9-444.yuv: ffmpeg's decode, see gen_fixtures_videocodecs_extra.sh)
#   vp8-vorbis.webm  VP8 with alt-ref frames, Vorbis stereo; .yuv
#   live.webm        VP8 written to a pipe: unknown-size Segment and Clusters, no Cues, no
#                    Duration; .yuv
#   hevc.mkv         HEVC Main; .yuv
#   av1.mkv          AV1; .yuv
#   sound.mka        Opus only (refused: no picture)
#   interlaced.m2ts  AVCHD-like: H.264 MBAFF, top field first, with real motion between the
#                    fields (50p woven to 25i), Blu-ray LPCM 16-bit stereo, 192-byte packets;
#                    .yuv (woven, as decoded), .pcm
#   ac3.mts          H.264 with AC-3 sound (silent, with a reason), 192-byte packets
#   hevc-wrap.ts     HEVC in 188-byte packets whose times wrap past 2^33 mid-clip; .yuv
set -euo pipefail
cd "$(dirname "$0")"
FF=(ffmpeg -hide_banner -loglevel error -y)
# Quadrants with a white 8x8 square moving left to right through the middle (as
# gen_fixtures.sh), at frame rate $1 for $2 seconds.
pattern() {
  echo "color=black:s=64x48:r=$1:d=$2,format=rgb24,geq=\
r='if(between(X,mod(N*2,56),mod(N*2,56)+7)*between(Y,20,27),255,if(lt(X,32)*lt(Y,24)+gte(X,32)*gte(Y,24),255,0))':\
g='if(between(X,mod(N*2,56),mod(N*2,56)+7)*between(Y,20,27),255,if(gte(X,32),255,0))':\
b='if(between(X,mod(N*2,56),mod(N*2,56)+7)*between(Y,20,27),255,if(gte(Y,24),255,0))'"
}
P25=$(pattern 25 0.8)
SINE="sine=frequency=1000:sample_rate=48000:duration=0.8"
X264=(-c:v libx264 -preset slow -pix_fmt yuv420p -x264-params keyint=8:min-keyint=8:scenecut=0:bframes=3:b-pyramid=normal)
VP=(-pix_fmt yuv420p -b:v 300k -g 8 -auto-alt-ref 1 -lag-in-frames 8 -deadline good -cpu-used 4)
yuv() { "${FF[@]}" -i "$1" -map 0:v -f rawvideo -pix_fmt "${3:-yuv420p}" "$2"; }

"${FF[@]}" -f lavfi -i "$P25" -f lavfi -i "$SINE" "${X264[@]}" \
  -c:a libopus -b:a 96k -ac 2 -application audio -shortest h264-opus.mkv
yuv h264-opus.mkv h264-opus.yuv
"${FF[@]}" -c:a libopus -i h264-opus.mkv -map 0:a -f s16le -ac 2 h264-opus.pcm

"${FF[@]}" -f lavfi -i "$P25" -f lavfi -i "$SINE" -c:v libvpx-vp9 "${VP[@]}" \
  -c:a libopus -b:a 16k -ac 1 -application voip -shortest vp9-opus.webm
yuv vp9-opus.webm vp9-opus.yuv
"${FF[@]}" -c:a libopus -i vp9-opus.webm -map 0:a -f s16le -ac 1 vp9-opus.pcm

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.4" -c:v libvpx-vp9 "${VP[@]}" \
  -pix_fmt yuv420p10le -profile:v 2 vp9-10bit.webm
yuv vp9-10bit.webm vp9-10bit.yuv yuv420p10le

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.2" -c:v libvpx-vp9 -pix_fmt yuv444p \
  -profile:v 1 vp9-444.webm

"${FF[@]}" -f lavfi -i "$P25" -f lavfi -i "$SINE" -c:v libvpx "${VP[@]}" \
  -c:a libvorbis -ac 2 -shortest vp8-vorbis.webm
yuv vp8-vorbis.webm vp8-vorbis.yuv

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.4" -c:v libvpx -pix_fmt yuv420p -g 5 -f webm pipe:1 > live.webm
yuv live.webm live.yuv

"${FF[@]}" -f lavfi -i "$P25" -c:v libx265 -pix_fmt yuv420p \
  -x265-params log-level=error:keyint=8:bframes=2 hevc.mkv
yuv hevc.mkv hevc.yuv

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.4" -c:v libaom-av1 -cpu-used 8 -g 5 \
  -pix_fmt yuv420p av1.mkv
yuv av1.mkv av1-mkv.yuv

"${FF[@]}" -f lavfi -i "sine=sample_rate=48000:duration=0.2" -c:a libopus sound.mka

# 50 progressive frames a second woven into 25 interlaced ones: each field its own moment.
"${FF[@]}" -f lavfi -i "$(pattern 50 0.8)" -f lavfi -i "$SINE" \
  -vf "tinterlace=interleave_top,setfield=tff" -c:v libx264 -pix_fmt yuv420p \
  -flags +ilme+ildct -x264-params tff=1:keyint=10:min-keyint=10:scenecut=0:bframes=2 \
  -c:a pcm_bluray -ac 2 -shortest -f mpegts -mpegts_m2ts_mode 1 interlaced.m2ts
yuv interlaced.m2ts interlaced.yuv
"${FF[@]}" -i interlaced.m2ts -map 0:a -f s16le interlaced.pcm

"${FF[@]}" -f lavfi -i "testsrc2=s=64x48:r=25:d=0.4" -f lavfi -i "$SINE" \
  -c:v libx264 -pix_fmt yuv420p -c:a ac3 -shortest -f mpegts -mpegts_m2ts_mode 1 ac3.mts

# Starts 0.2 s before the 33-bit PTS wrap (2^33 / 90 kHz = 95443.7176 s).
"${FF[@]}" -f lavfi -i "$P25" -c:v libx265 -pix_fmt yuv420p \
  -x265-params log-level=error:keyint=8:bframes=2 \
  -muxdelay 0 -muxpreload 0 -output_ts_offset 95443.5 -f mpegts hevc-wrap.ts
yuv hevc-wrap.ts hevc-wrap.yuv

# Four MP2 sound streams with ISO 639 descriptors whose audio_type comes from the
# disposition: main, audio description, hard of hearing, clean effects (ffprobe reads them).
"${FF[@]}" -f lavfi -i "testsrc=s=64x48:d=0.5:r=10" \
  -f lavfi -i "sine=f=440:d=0.5" -f lavfi -i "sine=f=660:d=0.5" \
  -f lavfi -i "sine=f=880:d=0.5" -f lavfi -i "sine=f=990:d=0.5" \
  -map 0 -map 1 -map 2 -map 3 -map 4 -c:v mpeg2video -c:a mp2 -b:a 64k \
  -metadata:s:a:0 language=eng -metadata:s:a:1 language=eng -disposition:a:1 visual_impaired \
  -metadata:s:a:2 language=deu -disposition:a:2 hearing_impaired \
  -metadata:s:a:3 language=eng -disposition:a:3 clean_effects ts-audio-types.ts

ls -l ./*.mkv ./*.webm ./*.mka ./*.m2ts ./*.mts ./*.ts ./*.pcm
# The HEVC above as a raw Annex B stream (tests/raw_streams.rs; oracle hevc.yuv).
"${FF[@]}" -i hevc.mkv -map 0:v -c:v copy -bsf:v hevc_mp4toannexb -f hevc es-bframes.hevc
