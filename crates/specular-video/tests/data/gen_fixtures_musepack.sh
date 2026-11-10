#!/bin/sh
# Musepack SV8 sound files from a generated tone, with libmpcdec r475's mpcdec as the
# oracle: .pcm (s16le, its float output truncated to 16 bits, delay and padding dropped).
# Run from this directory with mpcenc and mpcdec built from musepack_src_r475:
#   MPC=<folder with mpcenc and mpcdec> sh gen_fixtures_musepack.sh
#   snd-mpc-mono.mpc    0.5 s of 440 Hz at 44.1 kHz, --radio
#   snd-mpc-stereo.mpc  0.5 s of 440 Hz left, 660 Hz right, --standard
set -e
: "${MPC:?set MPC to the folder with mpcenc and mpcdec}"
tone() { python3 -I -c "
import math, struct, sys
ch = int(sys.argv[2]); n = 22050; out = bytearray()
for i in range(n):
    for c in range(ch):
        f = 440.0 if c == 0 else 660.0
        out += struct.pack('<h', int(round(8192 * math.sin(2 * math.pi * f * i / 44100))))
fmt = struct.pack('<IHHIIHH', 16, 1, ch, 44100, 44100 * 2 * ch, 2 * ch, 16)
open(sys.argv[1], 'wb').write(b'RIFF' + struct.pack('<I', 36 + len(out)) + b'WAVEfmt ' + fmt
    + b'data' + struct.pack('<I', len(out)) + out)
" "$@"; }
tone mono.wav 1
tone stereo.wav 2
"$MPC/mpcenc" --radio --overwrite mono.wav snd-mpc-mono.mpc
"$MPC/mpcenc" --standard --overwrite stereo.wav snd-mpc-stereo.mpc
for f in snd-mpc-mono snd-mpc-stereo; do
  "$MPC/mpcdec" $f.mpc $f.wav
  tail -c +45 $f.wav > $f.pcm
  rm $f.wav
done
rm mono.wav stereo.wav
