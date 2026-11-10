#!/bin/sh
# AC-3, E-AC-3, Vorbis and FLAC sound in their containers, Ogg (Theora) and IVF movies
# (ffmpeg 8.0 with libtheora, libvorbis, libvpx, libaom), with ffmpeg's decodes as the oracle:
# .yuv (every frame as decoded) and .pcm (s16le stereo at the stream's rate).
# Run from this directory: sh gen_fixtures_sound_in_containers.sh
#   theora-vorbis.ogv   Theora 4:2:0 with Vorbis stereo 44.1 kHz
#   theora-opus.ogv     Theora with Opus stereo
#   theora-flac.ogv     Theora with FLAC stereo 48 kHz (Ogg FLAC)
#   vp8-ac3.mkv         VP8 with AC-3 5.1 (folded to stereo)
#   mpeg4-eac3.mp4      MPEG-4 Part 2 with E-AC-3 stereo (`ec-3`)
#   mpeg4-ac3.mp4       MPEG-4 Part 2 with AC-3 stereo (`ac-3`)
#   mpeg4-flac.mp4      MPEG-4 Part 2 with FLAC (`fLaC`)
#   mpeg2-ac3.ts        MPEG-2 with AC-3 (stream type 0x81) in a transport stream
#   mpeg2-eac3.ts       MPEG-2 with E-AC-3 (stream type 0x87 / descriptor)
#   vp8-vorbis-mono.webm  VP8 with Vorbis mono
#   vp8-flac.mkv        VP8 with FLAC 5.1
#   dvd-ac3.pcm         the sound of dvd-ac3.vob (gen_fixtures_mpeg.sh)
#   ivf-vp8.ivf, ivf-vp9.ivf, ivf-av1.ivf  raw streams
set -e
q="-nostdin -loglevel error -y"
pic="-f lavfi -i testsrc2=size=96x64:rate=25:duration=0.4"
snd() { echo "-f lavfi -i sine=frequency=$1:sample_rate=$2:duration=0.4"; }
six="-f lavfi -i sine=frequency=300:sample_rate=48000:duration=0.4 -f lavfi -i sine=frequency=700:sample_rate=48000:duration=0.4 -f lavfi -i sine=frequency=1100:sample_rate=48000:duration=0.4 -f lavfi -i sine=frequency=60:sample_rate=48000:duration=0.4 -f lavfi -i sine=frequency=2300:sample_rate=48000:duration=0.4 -f lavfi -i sine=frequency=3700:sample_rate=48000:duration=0.4"
merge6="[1][2][3][4][5][6]amerge=inputs=6,volume=0.5,aformat=channel_layouts=5.1[a]"
ref() {
    ffmpeg $q -i "$1" -map 0:v:0 -fps_mode passthrough -f rawvideo "${1%.*}.yuv"
}
pcm() {
    ffmpeg $q -downmix stereo -i "$1" -map 0:a:0 -f s16le -ac 2 "${1%.*}.pcm"
}
ffmpeg $q $pic $(snd 500 44100) -ac 2 -c:v libtheora -q:v 6 -g 6 -c:a libvorbis -q:a 2 theora-vorbis.ogv
ref theora-vorbis.ogv; pcm theora-vorbis.ogv
ffmpeg $q $pic $(snd 600 48000) -ac 2 -c:v libtheora -q:v 6 -g 6 -c:a libopus -b:a 64k theora-opus.ogv
pcm theora-opus.ogv
ffmpeg $q $pic $(snd 700 48000) -ac 2 -c:v libtheora -q:v 6 -g 6 -c:a flac theora-flac.ogv
pcm theora-flac.ogv
ffmpeg $q $pic $six -filter_complex "$merge6" -map 0:v -map "[a]" -c:v libvpx -b:v 200k -g 6 -c:a ac3 -b:a 384k vp8-ac3.mkv
pcm vp8-ac3.mkv
ffmpeg $q $pic $(snd 800 48000) -ac 2 -c:v mpeg4 -q:v 4 -g 6 -c:a eac3 -b:a 128k mpeg4-eac3.mp4
pcm mpeg4-eac3.mp4
ffmpeg $q $pic $(snd 900 48000) -ac 2 -c:v mpeg4 -q:v 4 -g 6 -c:a ac3 -b:a 128k mpeg4-ac3.mp4
pcm mpeg4-ac3.mp4
ffmpeg $q $pic $(snd 1000 48000) -ac 2 -c:v mpeg4 -q:v 4 -g 6 -c:a flac -strict -2 mpeg4-flac.mp4
pcm mpeg4-flac.mp4
ffmpeg $q $pic $(snd 1100 48000) -ac 2 -c:v mpeg2video -q:v 4 -g 6 -c:a ac3 -b:a 128k -f mpegts mpeg2-ac3.ts
pcm mpeg2-ac3.ts
ffmpeg $q $pic $(snd 1200 48000) -ac 2 -c:v mpeg2video -q:v 4 -g 6 -c:a eac3 -b:a 128k -f mpegts mpeg2-eac3.ts
pcm mpeg2-eac3.ts
ffmpeg $q $pic $(snd 1300 48000) -ac 1 -c:v libvpx -b:v 200k -g 6 -c:a libvorbis -q:a 1 vp8-vorbis-mono.webm
pcm vp8-vorbis-mono.webm
ffmpeg $q $pic $six -filter_complex "$merge6" -map 0:v -map "[a]" -c:v libvpx -b:v 200k -g 6 -c:a flac vp8-flac.mkv
pcm vp8-flac.mkv
pcm dvd-ac3.vob
ffmpeg $q $pic -c:v libvpx -b:v 200k -g 6 ivf-vp8.ivf && ref ivf-vp8.ivf
ffmpeg $q $pic -c:v libvpx-vp9 -b:v 200k -g 6 ivf-vp9.ivf && ref ivf-vp9.ivf
ffmpeg $q $pic -c:v libaom-av1 -cpu-used 8 -b:v 200k -g 6 ivf-av1.ivf && ref ivf-av1.ivf
