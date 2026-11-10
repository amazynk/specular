#!/bin/sh
# A sound split into stream pieces (tests/sound_fx.rs, `pieces_go_on_with_one_decoder`):
#   snd-pieces.mp3  1.5 s of 440 Hz mono at 44.1 kHz, 64 kbit/s MP3 (bit reservoir in use),
#                   no Xing/LAME frame and no ID3 tag, so any frame boundary splits it.
# Run from this directory with ffmpeg built with libmp3lame.
set -e
ffmpeg -nostdin -hide_banner -loglevel error \
  -f lavfi -i "sine=frequency=440:duration=1.5:sample_rate=44100" \
  -ac 1 -c:a libmp3lame -b:a 64k -write_xing 0 -id3v2_version 0 \
  -fflags +bitexact -flags +bitexact -y snd-pieces.mp3
