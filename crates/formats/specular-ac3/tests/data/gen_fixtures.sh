#!/bin/sh
# Synthetic AC-3 and E-AC-3 streams (ffmpeg 8.0's encoders) and libavcodec's float decode of
# each as s16le: every channel (<name>.s16) and, for multichannel streams, its stereo downmix
# (<name>.st.s16). Run from this directory: sh gen_fixtures.sh
set -e
q="-nostdin -loglevel error -y"
# Tones per channel (different frequencies), a click train for block switching.
tone() { echo "sine=frequency=$1:sample_rate=$2:duration=0.3"; }
two() { echo "-f lavfi -i $(tone 440 $1) -f lavfi -i $(tone 1250 $1) -filter_complex [0][1]amerge=inputs=2,volume=0.7"; }
six() {
    r=$1
    echo "-f lavfi -i $(tone 300 $r) -f lavfi -i $(tone 700 $r) -f lavfi -i $(tone 1100 $r) -f lavfi -i $(tone 60 $r) -f lavfi -i $(tone 2300 $r) -f lavfi -i $(tone 3700 $r) -filter_complex [0][1][2][3][4][5]amerge=inputs=6,volume=0.5,aformat=channel_layouts=5.1"
}
ref() {
    ffmpeg $q -i "$1.$2" -f s16le "$1.s16"
}
refst() {
    ffmpeg $q -downmix stereo -i "$1.$2" -ac 2 -f s16le "$1.st.s16"
}
# AC-3 2/0, 48 kHz, 192 kbit/s (rematrixing, coupling off/on by the encoder's choice)
ffmpeg $q $(two 48000) -c:a ac3 -b:a 192k -f ac3 s48.ac3 && ref s48 ac3
# AC-3 2/0 with coupling forced, 44.1 kHz, 96 kbit/s
ffmpeg $q $(two 44100) -c:a ac3 -b:a 96k -channel_coupling 1 -f ac3 cpl44.ac3 && ref cpl44 ac3
# AC-3 1/0, 32 kHz, 64 kbit/s
ffmpeg $q -f lavfi -i "$(tone 523 32000)" -c:a ac3 -b:a 64k -f ac3 m32.ac3 && ref m32 ac3
# AC-3 3/2 + LFE, 48 kHz, 448 kbit/s, with Annex D mix levels
ffmpeg $q $(six 48000) -c:a ac3 -b:a 448k -center_mixlev 0.595 -surround_mixlev 0.5 -f ac3 c51.ac3 && ref c51 ac3 && refst c51 ac3
# AC-3 2/0 with transients (block switching): white-noise bursts
ffmpeg $q -f lavfi -i "aevalsrc=if(lt(mod(t\,0.1)\,0.004)\,0.8*sin(9000*t)\,0.02*sin(2000*t))|if(lt(mod(t+0.05\,0.1)\,0.004)\,0.8*sin(7000*t)\,0.02*sin(1500*t)):s=48000:d=0.3" -c:a ac3 -b:a 160k -f ac3 clicks.ac3 && ref clicks ac3
# E-AC-3 3/2 + LFE, 48 kHz, 384 kbit/s
ffmpeg $q $(six 48000) -c:a eac3 -b:a 384k -f eac3 e51.eac3 && ref e51 eac3 && refst e51 eac3
# E-AC-3 2/0, 44.1 kHz, 96 kbit/s, coupling
ffmpeg $q $(two 44100) -c:a eac3 -b:a 96k -f eac3 e44.eac3 && ref e44 eac3
# Undithered copies of four AC-3 streams (the `ac3_undither` example clears the dither
# flags, as fixtures.rs does again) and libavcodec's float decode of each as f32le
# (<name>.nd.f32; c51 folded to stereo, c51.nd.st.f32). With no random dither in either
# decoder the two agree to a fraction of a 16-bit step. UNDITHER names the example's
# binary (cargo build -p specular-ac3 --example ac3_undither).
nd() {
    "$UNDITHER" "$1.ac3" "$1.nd.ac3"
    if [ "$2" = st ]; then
        ffmpeg $q -downmix stereo -i "$1.nd.ac3" -ac 2 -f f32le "$1.nd.st.f32"
    else
        ffmpeg $q -i "$1.nd.ac3" -f f32le "$1.nd.f32"
    fi
    rm "$1.nd.ac3"
}
if [ -n "$UNDITHER" ]; then
    nd s48 && nd cpl44 && nd clicks && nd c51 st
fi
# E-AC-3 2/2, 48 kHz, 192 kbit/s: the extension channels of the two streams below
four() {
    r=$1
    echo "-f lavfi -i $(tone 500 $r) -f lavfi -i $(tone 900 $r) -f lavfi -i $(tone 1700 $r) -f lavfi -i $(tone 2900 $r) -filter_complex [0][1][2][3]amerge=inputs=4,volume=0.5,aformat=channel_layouts=quad"
}
ffmpeg $q $(four 48000) -c:a eac3 -b:a 192k -f eac3 q48.eac3
# Dependent substreams (no encoder at hand writes them): each e51 frame followed by the
# q48 frame as dependent substream 0 (`specular_ac3::as_dependent`): e71 maps its four
# channels to Ls Rs Lrs Rrs (7.1); e5q has no channel map (its L R Ls Rs replace e51's;
# libavcodec skips such a dependent substream, so e5q has no reference).
# Written by `SPECULAR_BLESS=1 cargo test -p specular-ac3 --test fixtures dependent`;
# then libavcodec's decode of each (7.1 in its order FL FR FC LFE BL BR SL SR).
if [ -f e71.eac3 ]; then
    ref e71 eac3
fi
