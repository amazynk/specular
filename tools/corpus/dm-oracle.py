#!/usr/bin/env python3
"""Data Matrix test corpus and oracle for specular-codes (not committed). Run in a venv with
pylibdmtx (needs the system libdmtx), zxing-cpp, pillow and numpy (on Python 3.12+ also
setuptools, for pylibdmtx's distutils import).

  gen <dir> [count]   writes <dir>/dm-NNNN.pgm and <dir>/manifest.tsv (file, expected bytes
                      as hex, size, encodation, distortion). Symbols come from libdmtx in
                      every encodation (ASCII, C40, Text, X12, EDIFACT, Base 256) and many
                      square and rectangular sizes; then rotation, perspective, noise, blur,
                      small modules, light-on-dark and mirroring.
  oracle <dir>        decodes every image with zxing-cpp and libdmtx; prints decoded/total
                      for each and writes <dir>/zxing.tsv and <dir>/dmtx.tsv (file, ok)
"""
import os
import random
import sys

import numpy as np
from PIL import Image, ImageFilter

SCHEMES = ['Ascii', 'C40', 'Text', 'X12', 'Edifact', 'Base256']
ALPHA = {
    'Ascii': 'abcdefghijklmnopqrstuvwxyz0123456789 .,-/:',
    'C40': 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ',
    'Text': 'abcdefghijklmnopqrstuvwxyz0123456789 ',
    'X12': 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 *>\r',
    'Edifact': 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 !"#$%&()*+,-./:;<=>?',
}
SQUARE = ['%dx%d' % (n, n) for n in (10, 12, 14, 16, 18, 20, 22, 24, 26, 32, 36, 40, 44, 48, 52, 64, 72, 80, 88, 96, 104, 120, 132, 144)]
RECT = ['8x18', '8x32', '12x26', '12x36', '16x36', '16x48']


def payload(r, scheme, cap):
    n = r.randint(1, max(1, cap))
    if scheme == 'Base256':
        return bytes(r.randint(0, 255) for _ in range(n))
    if scheme in ('C40', 'Text', 'X12'):
        n -= n % 3  # whole triples, so libdmtx keeps the scheme to the end
        n = max(n, 3)
    if scheme == 'Edifact':
        n -= n % 4
        n = max(n, 4)
    return ''.join(r.choice(ALPHA[scheme]) for _ in range(n)).encode('latin-1')


def matrix(e):
    """The module grid of a libdmtx encoding (5 px modules, 10 px margin)."""
    a = np.frombuffer(e.pixels, np.uint8).reshape(e.height, e.width, 3)[:, :, 0]
    rows, cols = (e.height - 20) // 5, (e.width - 20) // 5
    return a[12::5, 12::5][:rows, :cols] < 128


def make(r, i):
    from pylibdmtx import pylibdmtx as dmtx
    scheme = SCHEMES[i % len(SCHEMES)]
    rect = i % 5 == 3
    size = r.choice(RECT if rect else SQUARE[: (24 if i % 7 == 0 else 16)])
    sides = [int(x) for x in size.split('x')]
    cap = max(1, sides[0] * sides[1] // 16)
    for _ in range(12):
        data = payload(r, scheme, cap)
        try:
            e = dmtx.encode(data, scheme=scheme, size=size)
            break
        except Exception:
            cap = max(1, cap // 2)
    else:
        e = dmtx.encode(b'1', size=size)
        data = b'1'
    m = matrix(e)
    rows, cols = m.shape
    big = max(rows, cols)
    mod = r.choice([2, 3, 4, 5, 6, 8]) if big < 40 else r.choice([2, 3, 4])
    border = 3
    img = np.full(((rows + 2 * border) * mod, (cols + 2 * border) * mod), 255, np.uint8)
    for y in range(rows):
        for x in range(cols):
            if m[y, x]:
                img[(y + border) * mod:(y + border + 1) * mod, (x + border) * mod:(x + border + 1) * mod] = 0
    kind = []
    if i % 11 == 7:
        img = img[:, ::-1].copy()
        kind.append('mirror')
    if i % 13 == 5:
        img = 255 - img
        kind.append('invert')
    bg = 0 if 'invert' in kind else 255
    im = Image.fromarray(img)
    pad = max(im.width, im.height) // 2
    canvas = Image.new('L', (im.width + 2 * pad, im.height + 2 * pad), bg)
    canvas.paste(im, (pad, pad))
    im = canvas
    t = i % 5
    if t in (1, 3):
        ang = r.uniform(0, 360)
        im = im.rotate(ang, resample=Image.BILINEAR, fillcolor=bg)
        kind.append('rot%d' % ang)
    if t in (2, 3):
        w, h = im.size
        d = 0.12
        src = [(0, 0), (w, 0), (w, h), (0, h)]
        dst = [(x + r.uniform(-d, d) * w, y + r.uniform(-d, d) * h) for x, y in src]
        im = im.transform((w, h), Image.PERSPECTIVE, find_coeffs(dst, src), Image.BILINEAR, fillcolor=bg)
        kind.append('persp')
    if i % 4 == 1:
        im = im.filter(ImageFilter.GaussianBlur(r.uniform(0.5, 1.2) * mod / 3))
        kind.append('blur')
    a = np.asarray(im).astype(np.float32)
    if i % 3 == 2:
        s = r.uniform(10, 40)
        a = a + np.random.default_rng(i).normal(0, s, a.shape)
        kind.append('noise%d' % s)
    a = np.clip(a, 0, 255).astype(np.uint8)
    return a, data, '%dx%d' % (rows, cols), scheme, '+'.join(kind) or 'plain'


def find_coeffs(pa, pb):
    mat = []
    for p1, p2 in zip(pa, pb):
        mat.append([p1[0], p1[1], 1, 0, 0, 0, -p2[0] * p1[0], -p2[0] * p1[1]])
        mat.append([0, 0, 0, p1[0], p1[1], 1, -p2[1] * p1[0], -p2[1] * p1[1]])
    A = np.array(mat, dtype=np.float64)
    B = np.array(pb, dtype=np.float64).reshape(8)
    return np.linalg.solve(A, B).tolist()


def gen(d, count):
    os.makedirs(d, exist_ok=True)
    r = random.Random(16022)
    rows = []
    for i in range(count):
        a, data, size, scheme, kind = make(r, i)
        name = 'dm-%04d.pgm' % i
        h, w = a.shape
        with open(os.path.join(d, name), 'wb') as f:
            f.write(b'P5\n%d %d\n255\n' % (w, h))
            f.write(a.tobytes())
        rows.append('%s\t%s\t%s\t%s\t%s' % (name, data.hex(), size, scheme, kind))
    with open(os.path.join(d, 'manifest.tsv'), 'w') as f:
        f.write('\n'.join(rows) + '\n')
    print('wrote', count)


def oracle(d):
    import zxingcpp
    from pylibdmtx import pylibdmtx as dmtx
    rows = [l.rstrip('\n').split('\t') for l in open(os.path.join(d, 'manifest.tsv'))]
    zx, lib = [], []
    nz = nl = 0
    for name, hx, size, scheme, kind in rows:
        im = np.asarray(Image.open(os.path.join(d, name)))
        want = bytes.fromhex(hx)
        res = zxingcpp.read_barcodes(im, formats=zxingcpp.BarcodeFormat.DataMatrix, try_rotate=True,
                                     try_downscale=True, try_invert=True)
        z = any(bytes(x.bytes) == want for x in res)
        nz += z
        zx.append('%s\t%d' % (name, z))
        try:
            got = dmtx.decode(Image.fromarray(255 - im if 'invert' in kind else im), timeout=4000, max_count=1)
        except Exception:
            got = []
        l = any(g.data == want for g in got)
        nl += l
        lib.append('%s\t%d' % (name, l))
    with open(os.path.join(d, 'zxing.tsv'), 'w') as f:
        f.write('\n'.join(zx) + '\n')
    with open(os.path.join(d, 'dmtx.tsv'), 'w') as f:
        f.write('\n'.join(lib) + '\n')
    print('zxing-cpp: %d/%d decoded; libdmtx: %d/%d decoded' % (nz, len(rows), nl, len(rows)))


if __name__ == '__main__':
    if sys.argv[1] == 'gen':
        gen(sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 300)
    else:
        oracle(sys.argv[2])
