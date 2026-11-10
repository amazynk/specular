#!/usr/bin/env python3
"""QR code test corpus and oracle for specular-codes (not committed; run in a venv with
segno, zxing-cpp, pillow and numpy).

  gen <dir> [count]   writes <dir>/qr-NNNN.pgm and <dir>/manifest.tsv (file, expected text
                      as hex UTF-8, version, level, distortion) — rotations, perspective,
                      noise, blur, small modules, light-on-dark and mirrored symbols
  oracle <dir>        decodes every image with zxing-cpp; prints decoded/total and writes
                      <dir>/zxing.tsv (file, ok)
"""
import math
import os
import random
import sys

import numpy as np
import segno
from PIL import Image, ImageFilter

TEXTS = [
    lambda r: 'https://example.org/' + ''.join(r.choice('abcdefghijklmnopqrstuvwxyz0123456789') for _ in range(r.randint(1, 60))),
    lambda r: ''.join(r.choice('0123456789') for _ in range(r.randint(1, 300))),
    lambda r: ''.join(r.choice('ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 $%*+-./:') for _ in range(r.randint(1, 200))),
    lambda r: 'Grüße, ' + ''.join(r.choice('äöüßéèñ ') for _ in range(r.randint(1, 40))) + ' שלום',
    lambda r: ''.join(r.choice('点茗漢字日本語') for _ in range(r.randint(1, 30))),
    lambda r: ''.join(chr(r.randint(32, 126)) for _ in range(r.randint(100, 1200))),
]


def make(r, i):
    text = TEXTS[i % len(TEXTS)](r)
    level = r.choice('lmqh')
    kanji = i % len(TEXTS) == 4
    try:
        q = segno.make_qr(text, error=level, mode='kanji' if kanji else None, boost_error=False)
    except Exception:
        q = segno.make_qr(text, error=level, boost_error=False)
    mod = r.choice([2, 3, 4, 5, 6, 8]) if q.version < 20 else r.choice([2, 3, 4])
    border = 4
    m = np.array(q.matrix, dtype=np.uint8)
    n = m.shape[0]
    img = np.full(((n + 2 * border) * mod,) * 2, 255, np.uint8)
    for y in range(n):
        for x in range(n):
            if m[y, x]:
                img[(y + border) * mod:(y + border + 1) * mod, (x + border) * mod:(x + border + 1) * mod] = 0
    kind = []
    if i % 11 == 7:
        img = img[:, ::-1].copy()
        kind.append('mirror')
    if i % 13 == 5:
        img = 255 - img
        kind.append('invert')
    im = Image.fromarray(img)
    # pad so rotation and perspective keep it whole
    pad = im.width // 2
    canvas = Image.new('L', (im.width + 2 * pad, im.height + 2 * pad), 255 if 'invert' not in kind else 0)
    canvas.paste(im, (pad, pad))
    im = canvas
    t = i % 5
    if t in (1, 3):
        ang = r.uniform(0, 360)
        im = im.rotate(ang, resample=Image.BILINEAR, fillcolor=255 if 'invert' not in kind else 0)
        kind.append('rot%d' % ang)
    if t in (2, 3):
        w, h = im.size
        d = 0.12
        src = [(0, 0), (w, 0), (w, h), (0, h)]
        dst = [(x + r.uniform(-d, d) * w, y + r.uniform(-d, d) * h) for x, y in src]
        coeffs = find_coeffs(dst, src)
        im = im.transform((w, h), Image.PERSPECTIVE, coeffs, Image.BILINEAR, fillcolor=255 if 'invert' not in kind else 0)
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
    return a, text, q.version, level, '+'.join(kind) or 'plain'


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
    r = random.Random(18004)
    rows = []
    for i in range(count):
        a, text, v, level, kind = make(r, i)
        name = 'qr-%04d.pgm' % i
        h, w = a.shape
        with open(os.path.join(d, name), 'wb') as f:
            f.write(b'P5\n%d %d\n255\n' % (w, h))
            f.write(a.tobytes())
        rows.append('%s\t%s\t%d\t%s\t%s' % (name, text.encode('utf-8').hex(), v, level, kind))
    with open(os.path.join(d, 'manifest.tsv'), 'w') as f:
        f.write('\n'.join(rows) + '\n')
    print('wrote', count)


def oracle(d):
    import zxingcpp
    ok = 0
    out = []
    rows = [l.rstrip('\n').split('\t') for l in open(os.path.join(d, 'manifest.tsv'))]
    for name, hx, v, level, kind in rows:
        im = np.asarray(Image.open(os.path.join(d, name)))
        want = bytes.fromhex(hx).decode('utf-8')
        res = zxingcpp.read_barcodes(im, formats=zxingcpp.BarcodeFormat.QRCode, try_rotate=True, try_downscale=True, try_invert=True)
        good = any(x.text == want for x in res)
        ok += good
        out.append('%s\t%d' % (name, good))
    with open(os.path.join(d, 'zxing.tsv'), 'w') as f:
        f.write('\n'.join(out) + '\n')
    print('zxing-cpp: %d/%d decoded' % (ok, len(rows)))


if __name__ == '__main__':
    if sys.argv[1] == 'gen':
        gen(sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 300)
    else:
        oracle(sys.argv[2])
