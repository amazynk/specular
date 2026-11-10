"""mkyuv.py w h dx dy maxv out.yuv: synthetic planar 3-component raw image for ojph_compress,
the first component full size and the other two subsampled by (dx, dy); samples above 8 bits
are little-endian u16."""
import math, random, sys

w, h, dx, dy, maxv = map(int, sys.argv[1:6])
random.seed(3)
out = bytearray()
for c, (sx, sy) in enumerate([(1, 1), (dx, dy), (dx, dy)]):
    cw, ch = -(-w // sx), -(-h // sy)
    for y in range(ch):
        for x in range(cw):
            v = math.sin(x * sx * 0.04 * (c + 1) + y * sy * 0.05) * 0.4 + 0.5 + random.random() * 0.06 - 0.03
            v = int(max(0.0, min(1.0, v)) * maxv)
            out += bytes([v]) if maxv <= 255 else v.to_bytes(2, "little")
open(sys.argv[6], "wb").write(out)
