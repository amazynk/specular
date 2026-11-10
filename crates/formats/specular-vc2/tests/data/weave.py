"""Weaves FFmpeg's decode of field pictures (coded as half-height frames, since FFmpeg refuses
picture coding mode 1) into frames: the first, even-numbered field of each pair on the odd
lines (the custom base video format is bottom field first, Dirac specification Table C.1).

usage: python3 -I weave.py HALF.yuv OUT.yuv width height chroma(444/422/420) bytes_per_sample
"""

import sys

import numpy as np

src, dst, w, h, chroma, bps = sys.argv[1:]
w, h, bps = int(w), int(h), int(bps)
cw = w if chroma == "444" else (w + 1) // 2
fh = h // 2
cfh = fh if chroma != "420" else fh // 2
dt = np.uint8 if bps == 1 else np.dtype("<u2")
d = np.fromfile(src, dtype=dt)
planes = [(w, fh), (cw, cfh), (cw, cfh)]
field = sum(a * b for a, b in planes)
out = []
for f in range(len(d) // (2 * field)):
    pair = [d[(2 * f + k) * field:(2 * f + k + 1) * field] for k in range(2)]
    at = 0
    for pw, ph in planes:
        first = pair[0][at:at + pw * ph].reshape(ph, pw)
        second = pair[1][at:at + pw * ph].reshape(ph, pw)
        frame = np.empty((2 * ph, pw), dtype=dt)
        frame[1::2] = first
        frame[0::2] = second
        out.append(frame.flatten())
        at += pw * ph
np.concatenate(out).tofile(dst)
