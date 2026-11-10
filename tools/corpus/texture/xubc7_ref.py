"""XUBC7 reference BC7 blocks for every level of a KTX2 file, from the opengl_basisu wheel.

Usage: python -I xubc7_ref.py FILE.ktx2 OUTBASE
Writes OUTBASE.L<k>.blocks per level and prints level dims + slice marker/length.
"""

import struct
import sys

import numpy as np
from opengl_basisu import _transcoder as t

f, outbase = sys.argv[1], sys.argv[2]
data = open(f, 'rb').read()
levels = struct.unpack_from('<I', data, 40)[0]
sgd_off, sgd_len = struct.unpack_from('<QQ', data, 64)
fl = t.KTX2File(data)
if not fl.start():
    raise SystemExit('start failed')
for k in range(levels):
    loff, llen, _ = struct.unpack_from('<QQQ', data, 80 + 24 * k)
    so, sl, prof = struct.unpack_from('<III', data, sgd_off + 12 * k)
    first = data[loff + so]
    lw, lh, _ = fl.level(k, 0, 0)
    cols, rows = -(-lw // 4), -(-lh // 4)
    out = np.zeros(cols * rows * 16, dtype=np.uint8)
    if not fl.transcode(k, 0, 0, 6, out, cols * rows, 0, 0):
        raise SystemExit('transcode failed at level %d' % k)
    open('%s.L%d.blocks' % (outbase, k), 'wb').write(out.tobytes())
    open('%s.L%d.bin' % (outbase, k), 'wb').write(data[loff + so:loff + so + sl])
    print(k, lw, lh, 'slice', so, sl, 'profile %#x' % prof, 'first %#x' % first)
