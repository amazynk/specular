#!/usr/bin/env python3
"""DV with 12-bit nonlinear sound (IEC 61834-4, 32 kHz) for the specular-dv oracle: no
encoder writes it, so the 16-bit 48 kHz sound of an ffmpeg-made DV file (IN) is replaced:
the AAUX source packs say 32 kHz, 12 bits, and the audio DIF blocks carry two sine tones
compressed to 12-bit codes, three bytes per CH1/CH2 pair. The oracle is ffmpeg's decode of
OUT (`-map 0:a:0`, its own 12-bit expansion and de-shuffling).

Usage: python3 -I dv12-audio.py IN.dv OUT.dv   (NTSC or PAL, 25 Mb/s)
"""
import math
import sys

SEQ = 150 * 80


def compress(v):
    """A 16-bit sample to its 12-bit nonlinear code."""
    if v < 0:
        return ~compress(-v - 1) & 0xFFF
    if v < 512:
        return v
    s = v.bit_length() - 8  # v in [2^(s+7), 2^(s+8))
    return (s << 8) + ((v - (1 << (s + 7))) >> (s - 1))


data = bytearray(open(sys.argv[1], "rb").read())
pal = data[3] & 0x80 != 0
seqs = 12 if pal else 10
half = seqs // 2
frame = SEQ * seqs
minimum = 1264 if pal else 1053
t = 0
for f in range(len(data) // frame):
    base = f * frame
    n = minimum + (f % 3) * 7
    for s in range(seqs):
        for a in range(9):
            at = base + s * SEQ + (6 + a * 16) * 80
            blk = data[at:at + 80]
            if blk[3] == 0x50:
                blk[4] = (blk[4] & 0xC0) | (n - minimum)  # AF_SIZE
                blk[7] = (blk[7] & 0xC0) | (2 << 3) | 1  # 32 kHz, 12 bits
            for i in range(8, 80):
                blk[i] = 0
            data[at:at + 80] = blk
    for s in range(n):
        l = compress(int(4000 * math.sin(2 * math.pi * 440 * (t + s) / 32000)))
        r = compress(int(9000 * math.sin(2 * math.pi * 660 * (t + s) / 32000)))
        dseq = (s // 3 + 2 * (s % 3)) % half
        blk = 3 * (s % 3) + (s % (9 * half)) // (3 * half)
        byte = 8 + 3 * (s // (9 * half))
        at = base + dseq * SEQ + (6 + blk * 16) * 80 + byte
        data[at:at + 3] = bytes([l >> 4, r >> 4, ((l & 15) << 4) | (r & 15)])
    t += n
open(sys.argv[2], "wb").write(data)
