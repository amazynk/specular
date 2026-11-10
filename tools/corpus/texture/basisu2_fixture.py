"""Fixture: one level slice of an XUASTC KTX2 file plus the wheel's ASTC blocks for it."""
import struct, sys
import numpy as np
from opengl_basisu import _transcoder as t
src, level, target, out = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
d = open(src, "rb").read()
lv, sc = struct.unpack_from("<II", d, 40)
sgo, = struct.unpack_from("<Q", d, 64)
off, ln = struct.unpack_from("<QQ", d, 80 + 24 * level)
so, sl = struct.unpack_from("<II", d, sgo + 12 * level)
open(out + ".bin", "wb").write(d[off + so: off + so + sl])
f = t.KTX2File(d)
f.start()
w, h, _ = f.level(level, 0, 0)
bw, bh = t.target_block(target)
n = (-(-w // bw)) * (-(-h // bh))
o = np.zeros(n * 16, dtype=np.uint8)
assert f.transcode(level, 0, 0, target, o, n, 0, 0)
open(out + ".blocks", "wb").write(o.tobytes())
print(w, h, sl, n)
