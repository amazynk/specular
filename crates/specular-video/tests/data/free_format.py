"""Rewrites a constant-bit-rate MPEG audio stream as free format (bit-rate index 0 in every
header; the frames are unchanged), for gen_fixtures_sound_gaps.sh.
Usage: python3 -I free_format.py IN OUT"""

import sys

BR = {
    (1, 1): [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448],
    (1, 2): [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384],
    (1, 3): [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
    (2, 1): [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256],
    (2, 2): [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
}
BR[(2, 3)] = BR[(2, 2)]

d = bytearray(open(sys.argv[1], "rb").read())
at = 0
frames = 0
while at + 4 <= len(d):
    if d[at] != 0xFF or d[at + 1] & 0xE0 != 0xE0:
        at += 1
        continue
    h = int.from_bytes(d[at : at + 4], "big")
    ver = {3: 1, 2: 2, 0: 2}.get((h >> 19) & 3)
    layer = {3: 1, 2: 2, 1: 3}.get((h >> 17) & 3)
    bri, sri, pad = (h >> 12) & 15, (h >> 10) & 3, (h >> 9) & 1
    if ver is None or layer is None or bri in (0, 15) or sri == 3:
        at += 1
        continue
    rate = [44100, 48000, 32000][sri] >> {3: 0, 2: 1, 0: 2}[(h >> 19) & 3]
    br = BR[(ver, layer)][bri] * 1000
    if layer == 1:
        n = (12 * br // rate + pad) * 4
    elif layer == 3 and ver == 2:
        n = 72 * br // rate + pad
    else:
        n = 144 * br // rate + pad
    d[at + 2] &= 0x0F
    frames += 1
    at += n
open(sys.argv[2], "wb").write(d)
print(f"{frames} frames")
