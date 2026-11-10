#!/bin/sh
# An Opus stream with in-band FEC for the lost-packet tests (tests/soundfiles.rs), made with
# ffmpeg 8.0 (libopus): one second of voiced speech-like sound, SILK wideband, 20 ms packets,
# one packet per Ogg page (so a test can drop pages as a network stream would).
# Run from this directory: sh gen_fixtures_opus_fec.sh
set -e
q="-nostdin -loglevel error -y"
V="(0.2*sin(2*PI*(140+50*sin(1.3*t))*t)+0.12*sin(2*PI*2*(140+50*sin(1.3*t))*t)+0.08*sin(2*PI*3*(140+50*sin(1.3*t))*t)+0.05*sin(2*PI*5*(140+50*sin(1.3*t))*t))"
ffmpeg $q -f lavfi -i "aevalsrc=$V:s=48000:d=1" -c:a libopus -application voip -cutoff 8000 \
    -b:a 20k -frame_duration 20 -fec 1 -packet_loss 20 -page_duration 20000 \
    -map_metadata -1 -fflags +bitexact snd-opus-fec.opus
