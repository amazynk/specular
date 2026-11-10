"""Writes a sound-only MPEG program stream of 20 or 24-bit DVD LPCM (tones with a little
noise), in packets of an odd size so that sample groups straddle them, for
gen_fixtures_sound_gaps.sh. Usage: python3 -I lpcm_ps.py BITS CHANNELS RATE SECONDS OUT"""

import math
import random
import struct
import sys

bits, ch, rate = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
secs, out = float(sys.argv[4]), sys.argv[5]
rng = random.Random(7)
n = int(rate * secs) // 2 * 2
full = (1 << (bits - 1)) - 1
samples = [
    [
        int(full * (0.5 * math.sin(2 * math.pi * (300 + 200 * c) * i / rate) + 0.02 * (rng.random() - 0.5)))
        for c in range(ch)
    ]
    for i in range(n)
]
flat = [v for frame in samples for v in frame]
flat += [0] * (-len(flat) % 4)
stream = bytearray()
# Units of four samples in stream order (as ffmpeg's pcm_dvd writes and reads them).
for g in range(0, len(flat), 4):
    vals = flat[g : g + 4]
    for v in vals:
        stream += struct.pack(">h", v >> (bits - 16))
    if bits == 24:
        stream += bytes(v & 0xFF for v in vals)
    else:
        nib = [v & 15 for v in vals]
        stream += bytes((nib[i] << 4) | nib[i + 1] for i in range(0, len(nib), 2))


def pts_bytes(pts):
    return bytes(
        [
            0x21 | ((pts >> 29) & 0x0E),
            (pts >> 22) & 0xFF,
            0x01 | ((pts >> 14) & 0xFE),
            (pts >> 7) & 0xFF,
            0x01 | ((pts << 1) & 0xFE),
        ]
    )


def pack_header(scr):
    return bytes(
        [
            0, 0, 1, 0xBA,
            0x44 | ((scr >> 27) & 0x38) | ((scr >> 28) & 3),
            (scr >> 20) & 0xFF,
            0x04 | ((scr >> 12) & 0xF8) | ((scr >> 13) & 3),
            (scr >> 5) & 0xFF,
            0x04 | ((scr << 3) & 0xF8),
            0x01, 0x01, 0x89, 0xC3, 0xF8,
        ]
    )


q = {16: 0, 20: 1, 24: 2}[bits]
f = {48000: 0, 96000: 1}[rate]
chunk = 2001
data = bytearray()
pos = 0
while pos < len(stream):
    piece = stream[pos : pos + chunk]
    pts = 90000 + pos * 8 * 90000 // (ch * bits * rate)
    lpcm = bytes([0xA0, 1, 0, 4, 0, (q << 6) | (f << 4) | (ch - 1), 0x80])
    body = bytes([0x81, 0x80, 5]) + pts_bytes(pts) + lpcm + piece
    data += pack_header(pts) + bytes([0, 0, 1, 0xBD]) + struct.pack(">H", len(body)) + body
    pos += chunk
data += bytes([0, 0, 1, 0xB9])
open(out, "wb").write(data)
