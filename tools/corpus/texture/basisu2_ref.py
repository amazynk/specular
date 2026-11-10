"""Reference dumps for the Basis Universal v2 files, from the opengl_basisu wheel's transcoder.

Usage: python -I basisu2_ref.py DIR

For every .ktx2/.basis file in DIR, level 0 of face 0 / layer 0 is transcoded to the codec's
native GPU blocks (ASTC NxM for UASTC HDR 6x6i and XUASTC LDR, BC7 for XUBC7) and to pixels
(RGBA_HALF for HDR codecs, RGBA32 otherwise). Writes NAME.blocks (raw blocks, row-major) and
NAME.rgbah or NAME.rgba8, plus NAME.ref.txt (width, height, codec, block size).
Files the wheel cannot read are listed and skipped.
"""

import os
import sys

import numpy as np
from opengl_basisu import _transcoder as t

ASTC_LDR = {(4, 4): 10, (5, 4): 28, (5, 5): 29, (6, 5): 30, (6, 6): 31, (8, 5): 32, (8, 6): 33,
            (10, 5): 34, (10, 6): 35, (8, 8): 36, (10, 8): 37, (10, 10): 38, (12, 10): 39,
            (12, 12): 40}
XUASTC_DIMS = [(4, 4), (5, 4), (5, 5), (6, 5), (6, 6), (8, 5), (8, 6), (10, 5), (10, 6), (8, 8),
               (10, 8), (10, 10), (12, 10), (12, 12)]
ASTC_HDR_6X6, BC7, RGBA32, RGBA_HALF = 27, 6, 13, 25


def native(codec):
    if codec in (3, 4):
        return ASTC_HDR_6X6, (6, 6)
    if 5 <= codec <= 18:
        d = XUASTC_DIMS[codec - 5]
        return ASTC_LDR[d], d
    if 19 <= codec <= 32:
        d = XUASTC_DIMS[codec - 19]
        return ASTC_LDR[d], d
    if codec == 33:
        return BC7, (4, 4)
    return None, None


def run(f, data, kind):
    cls = t.KTX2File if kind == 'ktx2' else t.BasisFile
    fl = cls(data)
    w, h, levels, faces, layers, codec, alpha, srgb, video, yflip = fl.info()
    if not fl.start():
        raise RuntimeError('start failed')
    lw, lh, _ = fl.level(0, 0, 0)
    tgt, (bw, bh) = native(codec)
    base = os.path.splitext(f)[0] + ('' if kind == 'ktx2' else '.basis')
    if tgt is not None:
        cols, rows = -(-lw // bw), -(-lh // bh)
        out = np.zeros(cols * rows * 16, dtype=np.uint8)
        if not fl.transcode(0, 0, 0, tgt, out, cols * rows, 0, 0):
            raise RuntimeError('block transcode failed')
        open(base + '.blocks', 'wb').write(out.tobytes())
    px, ext, bpp = (RGBA_HALF, 'rgbah', 8) if codec in (2, 3, 4) else (RGBA32, 'rgba8', 4)
    out = np.zeros(lw * lh * bpp, dtype=np.uint8)
    if fl.transcode(0, 0, 0, px, out, lw * lh, lw, lh):
        open(base + '.' + ext, 'wb').write(out.tobytes())
    with open(base + '.ref.txt', 'w') as o:
        o.write('%d %d %d %d %d %d %d\n' % (lw, lh, codec, bw or 0, bh or 0, int(srgb), levels))
    return lw, lh, codec


def main():
    d = sys.argv[1]
    for n in sorted(os.listdir(d)):
        p = os.path.join(d, n)
        kind = 'ktx2' if n.endswith('.ktx2') else 'basis' if n.endswith('.basis') else None
        if kind is None:
            continue
        try:
            print(n, *run(p, open(p, 'rb').read(), kind))
        except Exception as e:  # noqa: BLE001 - report and carry on
            print(n, 'skipped:', e)


main()
