"""Packs the packets FFmpeg's image2 muxer split out into one fixture file:
u32 width, height, depth, then per packet u32 size and its bytes (big-endian).

    python3 -I pack.py OUT WIDTH HEIGHT DEPTH DIR
"""

import os
import struct
import sys

out, w, h, depth, folder = sys.argv[1:6]
data = struct.pack(">III", int(w), int(h), int(depth))
for name in sorted(os.listdir(folder)):
    p = open(os.path.join(folder, name), "rb").read()
    data += struct.pack(">I", len(p)) + p
open(out, "wb").write(data)
